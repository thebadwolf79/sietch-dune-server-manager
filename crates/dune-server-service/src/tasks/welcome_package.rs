use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::scheduler::{Schedule, Task, TaskCtx, TaskOutcome};
use crate::store::{WelcomeActionStatus, WelcomeGrantStatus};
use crate::tasks::TaskEnv;

const DEFAULT_CANDIDATE_LIMIT: u32 = 500;
const WELCOME_MESSAGE_ACTION_INDEX: i64 = -1;

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
    RefillWater {
        #[serde(rename = "waterAmount")]
        #[serde(default = "default_water_amount")]
        water_amount: i64,
        #[serde(
            rename = "delayAfterPreviousSecs",
            default = "default_refill_delay_secs"
        )]
        delay_after_previous_secs: u64,
    },
    SendWelcomeMessage,
}

impl WelcomePackageAction {
    fn action_type(&self) -> &'static str {
        match self {
            Self::GrantItem { .. } => "grant_item",
            Self::RefillWater { .. } => "refill_water",
            Self::SendWelcomeMessage => "send_welcome_message",
        }
    }

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
            Self::RefillWater {
                water_amount,
                delay_after_previous_secs,
            } => {
                if *water_amount <= 0 {
                    return Err(anyhow!(
                        "welcome refill water amount must be greater than 0"
                    ));
                }
                if *delay_after_previous_secs > 600 {
                    return Err(anyhow!("welcome refill delay must be <= 600 seconds"));
                }
                Ok(())
            }
            Self::SendWelcomeMessage => Ok(()),
        }
    }
}

fn default_water_amount() -> i64 {
    1_000_000
}

fn default_refill_delay_secs() -> u64 {
    30
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
    let has_action_tags = items.iter().any(|item| item.get("type").is_some());
    let actions = if has_action_tags {
        serde_json::from_value::<Vec<WelcomePackageAction>>(value)
            .map_err(|err| anyhow!("invalid welcome package action JSON: {err}"))?
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
        if self.env.welcome_package_enabled || self.env.welcome_message_enabled {
            Schedule::interval_secs(self.env.welcome_package_poll_secs)
        } else {
            Schedule::Disabled
        }
    }

    async fn run(&self, ctx: &TaskCtx) -> Result<TaskOutcome> {
        if !ctx.env.welcome_package_enabled && !ctx.env.welcome_message_enabled {
            ctx.log_info("welcome package and message disabled")?;
            return Ok(TaskOutcome::Done);
        }
        if ctx.env.welcome_package_enabled && ctx.env.welcome_package_actions.is_empty() {
            ctx.log_warn("welcome package enabled but action list is empty")?;
            if !ctx.env.welcome_message_enabled {
                return Ok(TaskOutcome::Done);
            }
        }

        let cluster = ctx.env.cluster.get().await?;
        let players = crate::postgres::list_welcome_candidates(
            &ctx.env.pg,
            &cluster.namespace,
            DEFAULT_CANDIDATE_LIMIT,
        )
        .await?;

        let mut seen = 0usize;
        let mut pending = 0usize;
        let mut granted = 0usize;
        let mut failed = 0usize;

        for player in players {
            if player.fls_id.trim().is_empty() {
                continue;
            }
            seen += 1;
            let record = ctx.store.ensure_welcome_grant(
                &player.fls_id,
                &ctx.env.welcome_package_version,
                player.account_id,
                player.character_name.as_deref(),
                &player.online_status,
            )?;

            if !player.online_status.eq_ignore_ascii_case("Online") {
                pending += 1;
                continue;
            }

            if !online_grace_elapsed(&record, ctx.env.welcome_package_online_grace_secs) {
                pending += 1;
                continue;
            }

            // Don't start the chain until the player actually has something
            // in any inventory. An empty pawn means the character hasn't
            // finished loading and MQ server-commands would be silently
            // dropped. Only enforced before the first action is published —
            // once the chain has started, confirmation runs to completion
            // even if the snapshot momentarily reads empty.
            if !ctx.store.welcome_package_chain_started(
                &player.fls_id,
                &ctx.env.welcome_package_version,
            )? {
                let inventory_total = crate::postgres::player_any_inventory_item_quantity(
                    &ctx.env.pg,
                    &cluster.namespace,
                    &player.fls_id,
                )
                .await?;
                if inventory_total <= 0 {
                    ctx.log_info(&format!(
                        "welcome package waiting for player inventory to populate player={}",
                        player.fls_id
                    ))?;
                    pending += 1;
                    continue;
                }
            }

            if ctx.dry_run {
                ctx.log_info(&format!(
                    "[dry-run] would run welcome package version={} player={} actions={}",
                    ctx.env.welcome_package_version,
                    player.fls_id,
                    configured_action_count(ctx.env.as_ref())
                ))?;
                continue;
            }

            match run_configured_actions(
                ctx,
                &cluster.namespace,
                &player.fls_id,
                &player.funcom_id,
                player.character_name.as_deref().unwrap_or(""),
            )
            .await
            {
                Ok(true) => {
                    if record.status != WelcomeGrantStatus::Granted {
                        ctx.store.mark_welcome_grant_granted(
                            &player.fls_id,
                            &ctx.env.welcome_package_version,
                        )?;
                    }
                    granted += 1;
                    ctx.log_info(&format!(
                        "welcome package granted player={} version={} actions={}",
                        player.fls_id,
                        ctx.env.welcome_package_version,
                        configured_action_count(ctx.env.as_ref())
                    ))?;
                }
                Ok(false) => {
                    pending += 1;
                }
                Err(err) => {
                    let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                    ctx.store.mark_welcome_grant_failed(
                        &player.fls_id,
                        &ctx.env.welcome_package_version,
                        &scrubbed,
                    )?;
                    failed += 1;
                    ctx.log_warn(&format!(
                        "welcome package failed player={} version={} error={}",
                        player.fls_id, ctx.env.welcome_package_version, scrubbed
                    ))?;
                }
            }
        }

        ctx.log_info(&format!(
            "welcome package scan complete seen={seen} pending={pending} granted={granted} failed={failed}"
        ))?;
        Ok(TaskOutcome::Done)
    }
}

fn configured_action_count(env: &TaskEnv) -> usize {
    env.welcome_package_actions.len() + usize::from(env.welcome_message_enabled)
}

fn online_grace_elapsed(record: &crate::store::WelcomeGrantRecord, grace_secs: u64) -> bool {
    if grace_secs == 0 {
        return true;
    }
    let Some(first_online_at) = record.first_online_at.as_deref() else {
        return false;
    };
    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(first_online_at) else {
        return false;
    };
    let elapsed = chrono::Utc::now()
        .signed_duration_since(parsed.with_timezone(&chrono::Utc))
        .num_seconds();
    elapsed >= grace_secs as i64
}

async fn run_configured_actions(
    ctx: &TaskCtx,
    namespace: &str,
    player_id: &str,
    recipient_funcom_id: &str,
    recipient_name: &str,
) -> Result<bool> {
    // Track whether every step is fully confirmed in this tick. Steps that
    // publish-and-await (grant_item between publish and Postgres readback,
    // refill_water before its delay elapses, welcome_message after publish)
    // flip this to false but the chain keeps going so all grant_item actions
    // fire in one scan tick instead of one-per-tick.
    let mut all_confirmed = true;

    if ctx.env.welcome_message_enabled {
        let record = ctx.store.ensure_welcome_action(
            player_id,
            &ctx.env.welcome_package_version,
            WELCOME_MESSAGE_ACTION_INDEX,
            "welcome_message",
        )?;
        if record.status != WelcomeActionStatus::Confirmed {
            match process_send_welcome_message(
                ctx,
                namespace,
                player_id,
                recipient_funcom_id,
                recipient_name,
                WELCOME_MESSAGE_ACTION_INDEX,
                &record,
            )
            .await
            {
                Ok(true) => {}
                Ok(false) => all_confirmed = false,
                Err(err) => {
                    let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                    ctx.store.mark_welcome_action_failed(
                        player_id,
                        &ctx.env.welcome_package_version,
                        WELCOME_MESSAGE_ACTION_INDEX,
                        &scrubbed,
                    )?;
                    return Err(err);
                }
            }
        }
    }

    if !ctx.env.welcome_package_enabled {
        return Ok(all_confirmed);
    }

    if ctx.env.welcome_package_require_empty_backpack
        && !ctx
            .store
            .welcome_package_chain_started(player_id, &ctx.env.welcome_package_version)?
    {
        let backpack_item_count =
            crate::postgres::player_backpack_item_quantity(&ctx.env.pg, namespace, player_id)
                .await?;
        if backpack_item_count > 0 {
            ctx.log_info(&format!(
                "welcome package waiting for empty backpack player={} items={}",
                player_id, backpack_item_count
            ))?;
            return Ok(false);
        }
    }

    for (index, action) in ctx.env.welcome_package_actions.iter().enumerate() {
        let index = index as i64;
        let record = ctx.store.ensure_welcome_action(
            player_id,
            &ctx.env.welcome_package_version,
            index,
            action.action_type(),
        )?;
        if record.status == WelcomeActionStatus::Confirmed {
            continue;
        }

        match action {
            WelcomePackageAction::GrantItem {
                item_name,
                quantity,
                durability,
            } => {
                match process_grant_item(
                    ctx,
                    namespace,
                    player_id,
                    index,
                    &record,
                    item_name,
                    *quantity,
                    *durability,
                )
                .await
                {
                    Ok(true) => {}
                    Ok(false) => all_confirmed = false,
                    Err(err) => {
                        let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                        ctx.store.mark_welcome_action_failed(
                            player_id,
                            &ctx.env.welcome_package_version,
                            index,
                            &scrubbed,
                        )?;
                        return Err(err);
                    }
                }
            }
            WelcomePackageAction::RefillWater {
                water_amount,
                delay_after_previous_secs,
            } => {
                match process_refill_water(
                    ctx,
                    player_id,
                    index,
                    &record,
                    *water_amount,
                    *delay_after_previous_secs,
                )
                .await
                {
                    Ok(true) => {}
                    Ok(false) => all_confirmed = false,
                    Err(err) => {
                        let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                        ctx.store.mark_welcome_action_failed(
                            player_id,
                            &ctx.env.welcome_package_version,
                            index,
                            &scrubbed,
                        )?;
                        return Err(err);
                    }
                }
            }
            WelcomePackageAction::SendWelcomeMessage => {
                match process_send_welcome_message(
                    ctx,
                    namespace,
                    player_id,
                    recipient_funcom_id,
                    recipient_name,
                    index,
                    &record,
                )
                .await
                {
                    Ok(true) => {}
                    Ok(false) => all_confirmed = false,
                    Err(err) => {
                        let scrubbed = crate::logger::redact(&format!("{err:#}")).into_owned();
                        ctx.store.mark_welcome_action_failed(
                            player_id,
                            &ctx.env.welcome_package_version,
                            index,
                            &scrubbed,
                        )?;
                        return Err(err);
                    }
                }
            }
        }
    }
    Ok(all_confirmed)
}

#[allow(clippy::too_many_arguments)]
async fn process_grant_item(
    ctx: &TaskCtx,
    namespace: &str,
    player_id: &str,
    index: i64,
    record: &crate::store::WelcomeActionRecord,
    item_name: &str,
    quantity: i64,
    durability: f64,
) -> Result<bool> {
    let baseline = match record.baseline_quantity {
        Some(value) => value,
        None => {
            crate::postgres::player_item_quantity(&ctx.env.pg, namespace, player_id, item_name)
                .await?
        }
    };
    let expected = record.expected_quantity.unwrap_or(baseline + quantity);

    let current =
        crate::postgres::player_item_quantity(&ctx.env.pg, namespace, player_id, item_name).await?;
    if current >= expected {
        ctx.store.mark_welcome_action_confirmed(
            player_id,
            &ctx.env.welcome_package_version,
            index,
        )?;
        ctx.log_info(&format!(
            "welcome action confirmed player={} version={} action={} item={} quantity={} expected={}",
            player_id, ctx.env.welcome_package_version, index, item_name, current, expected
        ))?;
        return Ok(true);
    }

    if record.published_at.is_some() {
        ctx.log_info(&format!(
            "welcome action waiting for item confirmation player={} version={} action={} item={} expected={}",
            player_id,
            ctx.env.welcome_package_version,
            index,
            item_name,
            expected
        ))?;
        return Ok(false);
    }

    let inner = json!({
        "ServerCommand": "AddItemToInventory",
        "PlayerId": player_id,
        "ItemName": item_name,
        "Quantity": quantity,
        "Durability": durability,
    });
    let result = ctx.env.mq.publish_inner(&inner, "welcome-package").await?;
    if !result.ok {
        return Err(anyhow!(
            "MQ publish did not report ok for item {}: {}",
            item_name,
            result.output.trim()
        ));
    }
    ctx.store.mark_welcome_action_published(
        player_id,
        &ctx.env.welcome_package_version,
        index,
        Some(item_name),
        Some(baseline),
        Some(expected),
    )?;
    let _ = ctx
        .store
        .record_admin_command("WelcomePackage.AddItemToInventory", &inner, true, None);

    // The grant was published. Don't block the scan loop polling Postgres
    // for confirmation — the early `current >= expected` check at the top
    // of this function will pick it up on the next tick.
    ctx.log_info(&format!(
        "welcome action published; awaiting confirmation on next tick player={} version={} action={} item={} expected={}",
        player_id, ctx.env.welcome_package_version, index, item_name, expected
    ))?;
    Ok(false)
}

async fn process_refill_water(
    ctx: &TaskCtx,
    player_id: &str,
    index: i64,
    record: &crate::store::WelcomeActionRecord,
    water_amount: i64,
    delay_after_previous_secs: u64,
) -> Result<bool> {
    if delay_after_previous_secs > 0 {
        let due = action_created_at(record)?
            + chrono::Duration::seconds(delay_after_previous_secs as i64);
        let now = chrono::Utc::now();
        if now < due {
            ctx.log_info(&format!(
                "waiting before water refill for player={} action={} due_in_secs={}",
                player_id,
                index,
                (due - now).num_seconds().max(0)
            ))?;
            return Ok(false);
        }
    }

    let inner = json!({
        "ServerCommand": "UpdateAllWaterFillables",
        "PlayerId": player_id,
        "WaterAmount": water_amount,
    });
    let result = ctx.env.mq.publish_inner(&inner, "welcome-package").await?;
    if !result.ok {
        return Err(anyhow!(
            "MQ publish did not report ok for water refill: {}",
            result.output.trim()
        ));
    }
    ctx.store.mark_welcome_action_published(
        player_id,
        &ctx.env.welcome_package_version,
        index,
        None,
        None,
        None,
    )?;
    ctx.store
        .mark_welcome_action_confirmed(player_id, &ctx.env.welcome_package_version, index)?;
    let _ = ctx.store.record_admin_command(
        "WelcomePackage.UpdateAllWaterFillables",
        &inner,
        true,
        None,
    );
    Ok(true)
}

async fn process_send_welcome_message(
    ctx: &TaskCtx,
    namespace: &str,
    recipient_fls_id: &str,
    recipient_funcom_id: &str,
    recipient_name: &str,
    index: i64,
    record: &crate::store::WelcomeActionRecord,
) -> Result<bool> {
    if record.published_at.is_some() {
        ctx.store.mark_welcome_action_confirmed(
            recipient_fls_id,
            &ctx.env.welcome_package_version,
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
        &ctx.env.welcome_package_version,
        index,
        None,
        None,
        None,
    )?;
    ctx.store.mark_welcome_action_confirmed(
        recipient_fls_id,
        &ctx.env.welcome_package_version,
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

fn action_created_at(
    record: &crate::store::WelcomeActionRecord,
) -> Result<chrono::DateTime<chrono::Utc>> {
    let parsed = chrono::DateTime::parse_from_rfc3339(&record.created_at)
        .map_err(|err| anyhow!("invalid welcome action timestamp: {err}"))?;
    Ok(parsed.with_timezone(&chrono::Utc))
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
    fn parse_welcome_actions_accepts_chain() {
        let actions = parse_welcome_actions(
            r#"[{"type":"grantItem","itemName":"Literjon"},{"type":"refillWater"}]"#,
        )
        .unwrap();
        assert_eq!(actions.len(), 2);
        assert!(matches!(actions[0], WelcomePackageAction::GrantItem { .. }));
        assert!(matches!(
            actions[1],
            WelcomePackageAction::RefillWater {
                delay_after_previous_secs: 30,
                ..
            }
        ));
    }

    #[test]
    fn parse_welcome_actions_accepts_water_delay() {
        let actions = parse_welcome_actions(
            r#"[{"type":"refillWater","waterAmount":3000,"delayAfterPreviousSecs":45}]"#,
        )
        .unwrap();
        assert!(matches!(
            actions[0],
            WelcomePackageAction::RefillWater {
                water_amount: 3000,
                delay_after_previous_secs: 45
            }
        ));
    }

    #[test]
    fn parse_welcome_actions_accepts_welcome_message() {
        let actions = parse_welcome_actions(r#"[{"type":"sendWelcomeMessage"}]"#).unwrap();
        assert!(matches!(
            actions[0],
            WelcomePackageAction::SendWelcomeMessage
        ));
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
    fn online_grace_requires_old_enough_timestamp() {
        let mut record = crate::store::WelcomeGrantRecord {
            player_id: "P1".into(),
            package_version: "v1".into(),
            account_id: 1,
            character_name: None,
            status: crate::store::WelcomeGrantStatus::Pending,
            detected_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
            granted_at: None,
            attempts: 0,
            last_online_status: Some("Online".into()),
            first_online_at: Some(chrono::Utc::now().to_rfc3339()),
            last_error: None,
        };
        assert!(!online_grace_elapsed(&record, 20));
        record.first_online_at =
            Some((chrono::Utc::now() - chrono::Duration::seconds(30)).to_rfc3339());
        assert!(online_grace_elapsed(&record, 20));
    }
}
