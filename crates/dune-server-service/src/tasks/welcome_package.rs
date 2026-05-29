use std::sync::Arc;

use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::store::WelcomeActionStatus;
use crate::tasks::TaskEnv;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

const WELCOME_PACKAGE_SCAN_INTERVAL_SECS: u64 = 2;
const WELCOME_MESSAGE_SCAN_INTERVAL_SECS: u64 = 60;
const WELCOME_MESSAGE_ACTION_INDEX: i64 = -1;
const WELCOME_MESSAGE_VERSION_SUFFIX: &str = ":message";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WelcomePackageItem {
    pub item_name: String,
    #[serde(default = "default_quantity")]
    pub quantity: i64,
    #[serde(default = "default_durability")]
    pub durability: f64,
}

impl WelcomePackageItem {
    pub fn validate(&self) -> Result<()> {
        if self.item_name.trim().is_empty() {
            return Err(anyhow!("welcome package itemName must not be empty"));
        }
        if self.quantity <= 0 {
            return Err(anyhow!(
                "welcome package quantity for {} must be greater than 0",
                self.item_name
            ));
        }
        if !self.durability.is_finite() || self.durability <= 0.0 {
            return Err(anyhow!(
                "welcome package durability for {} must be greater than 0",
                self.item_name
            ));
        }
        Ok(())
    }
}

fn default_quantity() -> i64 {
    1
}

fn default_durability() -> f64 {
    1.0
}

pub fn parse_welcome_items(raw: &str) -> Result<Vec<WelcomePackageItem>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let items: Vec<WelcomePackageItem> = serde_json::from_str(trimmed)
        .map_err(|err| anyhow!("invalid welcome package JSON: {err}"))?;
    for item in &items {
        item.validate()?;
    }
    Ok(items)
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WelcomePackageAction {
    GrantItem {
        #[serde(rename = "itemName")]
        item_name: String,
        #[serde(default = "default_quantity")]
        quantity: i64,
        #[serde(default = "default_durability")]
        durability: f64,
    },
}

impl WelcomePackageAction {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::GrantItem {
                item_name,
                quantity,
                durability,
            } => WelcomePackageItem {
                item_name: item_name.clone(),
                quantity: *quantity,
                durability: *durability,
            }
            .validate(),
        }
    }
}

pub fn parse_welcome_actions(raw: &str) -> Result<Vec<WelcomePackageAction>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|err| anyhow!("invalid welcome package JSON: {err}"))?;
    let Some(items) = value.as_array() else {
        return Err(anyhow!("welcome package JSON must be an array"));
    };
    let actions: Vec<WelcomePackageAction> = if items.iter().any(|item| item.get("type").is_some())
    {
        items
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(serde_json::Value::as_str) != Some("grantItem") {
                    return None;
                }
                serde_json::from_value::<WelcomePackageAction>(item.clone()).ok()
            })
            .collect()
    } else {
        serde_json::from_value::<Vec<WelcomePackageItem>>(value)
            .map_err(|err| anyhow!("invalid welcome package item JSON: {err}"))?
            .into_iter()
            .map(|item| WelcomePackageAction::GrantItem {
                item_name: item.item_name,
                quantity: item.quantity,
                durability: item.durability,
            })
            .collect()
    };
    for action in &actions {
        action.validate()?;
    }
    Ok(actions)
}

pub struct WelcomePackageTask {
    env: Arc<TaskEnv>,
}

impl WelcomePackageTask {
    pub fn new(env: Arc<TaskEnv>) -> Self {
        Self { env }
    }
}

#[async_trait]
impl Task for WelcomePackageTask {
    fn id(&self) -> &'static str {
        "welcome-package"
    }

    fn schedule(&self) -> Schedule {
        if self.env.welcome_package_enabled {
            Schedule::interval_secs(WELCOME_PACKAGE_SCAN_INTERVAL_SECS)
        } else {
            Schedule::Disabled
        }
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        if !ctx.env.welcome_package_enabled {
            ctx.log_info("welcome package disabled")?;
            return Ok(TaskOutcome::Done);
        }
        if ctx.env.welcome_package_actions.is_empty() {
            ctx.log_warn("welcome package enabled but action list is empty")?;
            return Ok(TaskOutcome::Done);
        }

        let cluster = ctx.env.cluster.get().await?;
        let accounts =
            crate::postgres::list_welcome_accounts(&ctx.env.pg, &cluster.namespace).await?;

        for account in accounts {
            if account.fls_id.trim().is_empty() {
                continue;
            }

            if ctx.dry_run {
                ctx.log_info(&format!(
                    "[dry-run] would inspect welcome package version={} player={} account_id={} items={}",
                    ctx.env.welcome_package_version,
                    account.fls_id,
                    account.account_id,
                    ctx.env.welcome_package_actions.len()
                ))?;
                continue;
            }

            // Cheap sqlite gate: a granted OR failed ledger row means we are
            // done with this account. Failed rows are cleared by the operator
            // via the "retry" action, which deletes the row so it re-attempts.
            if ctx.store.welcome_grant_exists(
                &account.fls_id,
                &ctx.env.welcome_package_version,
                account.account_id,
            )? {
                continue;
            }

            match process_item_package(ctx, &cluster.namespace, &account).await {
                Ok(Some(character_name)) => {
                    ctx.store.insert_welcome_grant_granted(
                        &account.fls_id,
                        &ctx.env.welcome_package_version,
                        account.account_id,
                        Some(character_name.as_str()),
                    )?;
                }
                // Backpack inventory row not present yet — leave no ledger row
                // so a later scan retries once the character finishes loading.
                Ok(None) => {}
                Err(err) => {
                    let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                    ctx.store.insert_welcome_grant_failed(
                        &account.fls_id,
                        &ctx.env.welcome_package_version,
                        account.account_id,
                        None,
                        &scrubbed,
                    )?;
                    ctx.log_warn(&format!(
                        "welcome package failed player={} account_id={} version={} error={}",
                        account.fls_id, account.account_id, ctx.env.welcome_package_version, scrubbed
                    ))?;
                }
            }
        }

        Ok(TaskOutcome::Done)
    }
}

/// Sends the welcome whisper on its own slower cadence. Split out from the
/// package worker so the 2s item-grant scan does not hit Postgres for a chat
/// lookup on every account every tick.
pub struct WelcomeMessageTask {
    env: Arc<TaskEnv>,
}

impl WelcomeMessageTask {
    pub fn new(env: Arc<TaskEnv>) -> Self {
        Self { env }
    }
}

#[async_trait]
impl Task for WelcomeMessageTask {
    fn id(&self) -> &'static str {
        "welcome-message"
    }

    fn schedule(&self) -> Schedule {
        if self.env.welcome_message_enabled {
            Schedule::interval_secs(WELCOME_MESSAGE_SCAN_INTERVAL_SECS)
        } else {
            Schedule::Disabled
        }
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        if !ctx.env.welcome_message_enabled {
            ctx.log_info("welcome message disabled")?;
            return Ok(TaskOutcome::Done);
        }

        let cluster = ctx.env.cluster.get().await?;
        let accounts =
            crate::postgres::list_welcome_accounts(&ctx.env.pg, &cluster.namespace).await?;
        let message_version = format!(
            "{}{}",
            ctx.env.welcome_package_version, WELCOME_MESSAGE_VERSION_SUFFIX
        );

        for account in accounts {
            if account.fls_id.trim().is_empty() {
                continue;
            }

            if ctx.dry_run {
                ctx.log_info(&format!(
                    "[dry-run] would send welcome whisper player={} account_id={}",
                    account.fls_id, account.account_id
                ))?;
                continue;
            }

            // Cheap sqlite gate before any Postgres chat lookup: skip accounts
            // whose whisper is already confirmed.
            if ctx.store.welcome_action_confirmed(
                &account.fls_id,
                &message_version,
                account.account_id,
                WELCOME_MESSAGE_ACTION_INDEX,
            )? {
                continue;
            }

            if let Err(err) = process_account_welcome_message(ctx, &cluster.namespace, &account).await
            {
                let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                ctx.log_warn(&format!(
                    "welcome message failed player={} account_id={} error={}",
                    account.fls_id, account.account_id, scrubbed
                ))?;
            }
        }

        Ok(TaskOutcome::Done)
    }
}

async fn process_item_package(
    ctx: &TaskCtx,
    namespace: &str,
    account: &crate::postgres::WelcomeAccount,
) -> Result<Option<String>> {
    let Some(backpack) =
        crate::postgres::resolve_account_backpack(&ctx.env.pg, namespace, account.account_id)
            .await?
    else {
        return Ok(None);
    };

    let items = ctx
        .env
        .welcome_package_actions
        .iter()
        .map(|action| match action {
            WelcomePackageAction::GrantItem {
                item_name,
                quantity,
                durability,
            } => Ok(crate::postgres::BackpackGrantItem {
                template_id: item_name.clone(),
                quantity: *quantity,
                stats_json: welcome_item_stats_json(item_name, *durability)?,
            }),
        })
        .collect::<Result<Vec<_>>>()?;

    let ids = crate::postgres::insert_items_to_backpack(
        &ctx.env.pg,
        namespace,
        backpack.inventory_id,
        &items,
    )
    .await?;
    let _ = ctx.store.record_admin_command(
        "WelcomePackage.DbAddItemsToBackpack",
        &json!({
            "playerId": account.fls_id,
            "accountId": account.account_id,
            "inventoryId": backpack.inventory_id,
            "items": ctx.env.welcome_package_actions,
            "itemIds": ids,
        }),
        true,
        None,
    );
    ctx.log_info(&format!(
        "welcome package db-confirmed player={} account_id={} inventory_id={} version={} items={} item_ids={:?}",
        account.fls_id,
        account.account_id,
        backpack.inventory_id,
        ctx.env.welcome_package_version,
        items.len(),
        ids
    ))?;
    Ok(Some(backpack.character_name.unwrap_or_default()))
}

async fn process_account_welcome_message(
    ctx: &TaskCtx,
    namespace: &str,
    account: &crate::postgres::WelcomeAccount,
) -> Result<bool> {
    let message_version = format!(
        "{}{}",
        ctx.env.welcome_package_version, WELCOME_MESSAGE_VERSION_SUFFIX
    );
    let recipient = match crate::postgres::resolve_chat_player(
        &ctx.env.pg,
        namespace,
        &account.fls_id,
    )
    .await?
    {
        Some(recipient) => recipient,
        None => return Ok(false),
    };
    ctx.store.ensure_welcome_grant(
        &account.fls_id,
        &message_version,
        account.account_id,
        Some(&recipient.character_name),
        "",
    )?;
    let record = ctx.store.ensure_welcome_action(
        &account.fls_id,
        &message_version,
        account.account_id,
        WELCOME_MESSAGE_ACTION_INDEX,
        "welcome_message",
    )?;
    if record.status == WelcomeActionStatus::Confirmed {
        return Ok(false);
    }
    process_send_welcome_message(
        ctx,
        namespace,
        &recipient.fls_id,
        &recipient.funcom_id,
        account.account_id,
        &recipient.character_name,
        &message_version,
        WELCOME_MESSAGE_ACTION_INDEX,
        &record,
    )
    .await
}

async fn process_send_welcome_message(
    ctx: &TaskCtx,
    namespace: &str,
    recipient_fls_id: &str,
    recipient_funcom_id: &str,
    account_id: i64,
    recipient_name: &str,
    package_version: &str,
    index: i64,
    record: &crate::store::WelcomeActionRecord,
) -> Result<bool> {
    if record.published_at.is_some() {
        ctx.store.mark_welcome_action_confirmed(
            recipient_fls_id,
            package_version,
            account_id,
            index,
        )?;
        return Ok(true);
    }

    let source_lookup = ctx.env.welcome_whisper_source_player.trim();
    let message = ctx.env.welcome_message.trim();
    if message.is_empty() {
        return Err(anyhow!("welcome message must not be empty"));
    }
    if recipient_funcom_id.trim().is_empty() {
        return Err(anyhow!(
            "recipient player {} does not have a Funcom chat id",
            recipient_fls_id
        ));
    }
    let source_lookup = if source_lookup.is_empty() {
        recipient_fls_id
    } else {
        source_lookup
    };

    let source = crate::postgres::resolve_chat_player(&ctx.env.pg, namespace, source_lookup)
        .await?
        .ok_or_else(|| anyhow!("welcome whisper source player not found: {source_lookup}"))?;
    let result = publish_welcome_whisper(
        &ctx.env.mq,
        &source,
        recipient_fls_id,
        recipient_funcom_id,
        recipient_name,
        message,
        "welcome-whisper",
    )
    .await?;
    let _ = ctx.store.record_admin_command(
        "WelcomePackage.SendWelcomeWhisper",
        &json!({
            "sourcePlayerId": source.fls_id,
            "recipientPlayerId": recipient_fls_id,
            "recipientFuncomId": recipient_funcom_id,
            "message": message,
        }),
        result.ok,
        None,
    );
    ctx.store.mark_welcome_action_published(
        recipient_fls_id,
        package_version,
        account_id,
        index,
        None,
        None,
        None,
    )?;
    ctx.store.mark_welcome_action_confirmed(
        recipient_fls_id,
        package_version,
        account_id,
        index,
    )?;
    Ok(true)
}

pub async fn send_welcome_whisper_now(
    env: &TaskEnv,
    namespace: &str,
    source_lookup: &str,
    recipient_lookup: &str,
    message: &str,
) -> Result<crate::admin::PublishResult> {
    let recipient = crate::postgres::resolve_chat_player(&env.pg, namespace, recipient_lookup)
        .await?
        .ok_or_else(|| anyhow!("recipient player not found: {recipient_lookup}"))?;
    let source_lookup = if source_lookup.trim().is_empty() {
        recipient.fls_id.as_str()
    } else {
        source_lookup.trim()
    };
    let source = crate::postgres::resolve_chat_player(&env.pg, namespace, source_lookup)
        .await?
        .ok_or_else(|| anyhow!("welcome whisper source player not found: {source_lookup}"))?;
    publish_welcome_whisper(
        &env.mq,
        &source,
        &recipient.fls_id,
        &recipient.funcom_id,
        &recipient.character_name,
        message.trim(),
        "welcome-whisper",
    )
    .await
}

async fn publish_welcome_whisper(
    mq: &crate::admin::MqPublisher,
    source: &crate::postgres::ChatPlayer,
    recipient_fls_id: &str,
    recipient_funcom_id: &str,
    recipient_name: &str,
    message: &str,
    label: &str,
) -> Result<crate::admin::PublishResult> {
    if message.trim().is_empty() {
        return Err(anyhow!("welcome message must not be empty"));
    }
    if source.fls_id.trim().is_empty() || source.funcom_id.trim().is_empty() {
        return Err(anyhow!(
            "welcome whisper source player has incomplete chat identity"
        ));
    }
    if recipient_funcom_id.trim().is_empty() {
        return Err(anyhow!(
            "recipient player {} does not have a Funcom chat id",
            recipient_fls_id
        ));
    }

    let body = build_whisper_body(
        &source.funcom_id,
        recipient_funcom_id,
        recipient_name,
        message,
    )?;
    let result = mq
        .publish_whisper(recipient_funcom_id, &source.fls_id, &body, label)
        .await?;
    if !result.ok {
        return Err(anyhow!(
            "MQ publish did not report ok for welcome whisper: {}",
            result.output.trim()
        ));
    }
    Ok(result)
}

fn build_whisper_body(
    sender_funcom_id: &str,
    recipient_funcom_id: &str,
    recipient_name: &str,
    message: &str,
) -> Result<serde_json::Value> {
    let message = normalize_chat_message(message);
    let chat = json!({
        "m_Id": message_guid(sender_funcom_id, recipient_funcom_id),
        "m_ChannelType": "ETextChatChannelType::Whispers",
        "m_SubChannelId": recipient_funcom_id,
        "m_bUseSpoofedUserName": false,
        "m_SpoofedUserNameFrom": {"m_Id": "", "m_DisplayName": ""},
        "m_FuncomIdFrom": sender_funcom_id,
        "m_UserNameTo": recipient_name,
        "m_Message": {
            "m_UnlocalizedMessage": message,
            "m_LocalizedMessage": {"m_TableId": "", "m_Key": "", "m_FormatArgs": []}
        },
        "m_TimeStamp": chrono::Utc::now().to_rfc3339(),
        "m_OriginLocation": {"X": 0.0, "Y": 0.0, "Z": 0.0},
        "m_HasSeenMessage": false
    });
    Ok(json!({
        "Content": serde_json::to_string(&chat)?,
        "Type": "ECourierMessageType::TextChat"
    }))
}

fn normalize_chat_message(message: &str) -> String {
    message
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn welcome_item_stats_json(item_name: &str, durability: f64) -> Result<String> {
    let durability_stats = if durability >= 1.0 {
        json!({"DecayedMaxDurability": 0.0})
    } else {
        json!({"CurrentDurability": durability})
    };
    let mut stats = serde_json::Map::from_iter([(
        "FItemStackAndDurabilityStats".to_string(),
        json!([[], durability_stats]),
    )]);
    if let Some(max_amount) = fillable_water_container_max_amount(item_name) {
        stats.insert(
            "FFillableItemStats".to_string(),
            json!([[], {
                "CurrentAmount": max_amount as f64,
                "MaxAmount": max_amount,
                "FillableType": "Water",
                "FillableTypeRestriction": "Water",
                "bIsContainer": true,
            }]),
        );
    }
    Ok(serde_json::to_string(&serde_json::Value::Object(stats))?)
}

fn fillable_water_container_max_amount(item_name: &str) -> Option<i64> {
    match item_name.to_ascii_lowercase().as_str() {
        // Dune/Systems/Items/DT_ItemTableFillables container rows.
        "dewpack" => Some(250),
        "literjon" => Some(1000),
        "highcapacityliterjon" => Some(1500),
        "literjon_03" => Some(1100),
        "literjon_04" => Some(1200),
        "literjon_05" => Some(1300),
        "literjon_06" => Some(1400),
        "literjon_07" => Some(1500),
        "literjon_08" => Some(1600),
        "literjon_09" => Some(1700),
        "decajon" => Some(10_000),
        "literjon_t6" => Some(20_000),
        "highcapacityliterjon_02" => Some(1750),
        "highcapacityliterjon_03" => Some(2000),
        "highcapacityliterjon_04" => Some(2250),
        "highcapacityliterjon_05" => Some(2500),
        "highcapacityliterjon_06" => Some(3000),
        _ => None,
    }
}

fn message_guid(sender_funcom_id: &str, recipient_funcom_id: &str) -> String {
    use std::hash::{Hash, Hasher};
    let nanos = chrono::Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() * 1_000_000);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sender_funcom_id.hash(&mut hasher);
    recipient_funcom_id.hash(&mut hasher);
    nanos.hash(&mut hasher);
    let a = nanos as u128;
    let b = hasher.finish() as u128;
    let raw = (a << 64) ^ b;
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (raw >> 96) as u32,
        (raw >> 80) as u16,
        (raw >> 64) as u16,
        (raw >> 48) as u16,
        raw & 0xffff_ffff_ffff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_welcome_items_accepts_defaults() {
        let items = parse_welcome_items(r#"[{"itemName":"PlantFiber"}]"#).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_name, "PlantFiber");
        assert_eq!(items[0].quantity, 1);
        assert_eq!(items[0].durability, 1.0);
    }

    #[test]
    fn parse_welcome_actions_accepts_item_rows() {
        let actions =
            parse_welcome_actions(r#"[{"type":"grantItem","itemName":"Literjon"}]"#).unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], WelcomePackageAction::GrantItem { .. }));
    }

    #[test]
    fn parse_welcome_actions_drops_legacy_non_item_actions() {
        let actions = parse_welcome_actions(
            r#"[{"type":"refillWater"},{"type":"sendWelcomeMessage"},{"type":"grantItem","itemName":"PlantFiber"}]"#,
        )
        .unwrap();
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], WelcomePackageAction::GrantItem { .. }));
    }

    #[test]
    fn whisper_body_uses_localizable_message_shape() {
        let body = build_whisper_body("sender-chat", "recipient-chat", "Ada", "Welcome").unwrap();
        assert_eq!(body["Type"], "ECourierMessageType::TextChat");
        let content: serde_json::Value =
            serde_json::from_str(body["Content"].as_str().unwrap()).unwrap();
        assert_eq!(content["m_ChannelType"], "ETextChatChannelType::Whispers");
        assert_eq!(content["m_SubChannelId"], "recipient-chat");
        assert_eq!(content["m_Message"]["m_UnlocalizedMessage"], "Welcome");
        assert!(content["m_Message"]["m_LocalizedMessage"]["m_FormatArgs"].is_array());
    }

    #[test]
    fn whisper_body_flattens_multiline_text() {
        let body = build_whisper_body("sender-chat", "recipient-chat", "Ada", "Hello\r\n\nArrakis")
            .unwrap();
        let content: serde_json::Value =
            serde_json::from_str(body["Content"].as_str().unwrap()).unwrap();
        assert_eq!(
            content["m_Message"]["m_UnlocalizedMessage"],
            "Hello Arrakis"
        );
    }

    #[test]
    fn parse_welcome_actions_keeps_old_item_json_compatible() {
        let actions = parse_welcome_actions(r#"[{"itemName":"PlantFiber","quantity":2}]"#).unwrap();
        assert_eq!(
            actions[0],
            WelcomePackageAction::GrantItem {
                item_name: "PlantFiber".into(),
                quantity: 2,
                durability: 1.0
            }
        );
    }

    #[test]
    fn parse_welcome_items_rejects_bad_quantity() {
        let err = parse_welcome_items(r#"[{"itemName":"PlantFiber","quantity":0}]"#).unwrap_err();
        assert!(err.to_string().contains("quantity"));
    }

    #[test]
    fn literjon_stats_are_inserted_full() {
        let stats: serde_json::Value =
            serde_json::from_str(&welcome_item_stats_json("Literjon", 1.0).unwrap()).unwrap();
        assert_eq!(stats["FFillableItemStats"][1]["CurrentAmount"], 1000.0);
        assert_eq!(stats["FFillableItemStats"][1]["MaxAmount"], 1000);
        assert_eq!(stats["FFillableItemStats"][1]["FillableType"], "Water");
        assert_eq!(
            stats["FFillableItemStats"][1]["FillableTypeRestriction"],
            "Water"
        );
        assert_eq!(stats["FFillableItemStats"][1]["bIsContainer"], true);
    }

    #[test]
    fn decajon_stats_are_inserted_full() {
        let stats: serde_json::Value =
            serde_json::from_str(&welcome_item_stats_json("Decajon", 1.0).unwrap()).unwrap();
        assert_eq!(stats["FFillableItemStats"][1]["CurrentAmount"], 10000.0);
        assert_eq!(stats["FFillableItemStats"][1]["MaxAmount"], 10000);
    }
}
