use super::{BuildKind, Category, CommandSpec, FieldKind, FieldSpec, SelectOption};

const FIELD_PLAYER: FieldSpec = FieldSpec {
    key: "PlayerId",
    label: "Player",
    kind: FieldKind::String,
    required: Some(true),
    default: None,
    helper: Some("FLS player id, or \"*\" for all online"),
    options: None,
};

// Journey commands removed 2026-05-26: published successfully but the
// server-command handlers don't apply the state changes (live-tested).
// `FIELD_STORY_NODE` + `PLAYER_AND_STORY` retired with them.

// XP category options were removed 2026-05-26 — live-testing showed the
// server ignores Category and always grants generic player XP regardless of
// which value is sent. Keeping AwardXP as a player+amount command only.

const BROADCAST_TYPES: &[SelectOption] = &[
    SelectOption {
        value: "Generic",
        label: "Generic",
    },
    SelectOption {
        value: "ServerShutdown",
        label: "ServerShutdown",
    },
];

const ADD_ITEM_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "ItemName",
        label: "ItemName",
        kind: FieldKind::String,
        required: Some(true),
        default: None,
        helper: Some("Internal FName, case-insensitive"),
        options: None,
    },
    FieldSpec {
        key: "Quantity",
        label: "Quantity",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(1)),
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Durability",
        label: "Durability",
        kind: FieldKind::Float,
        required: None,
        default: Some(json_const_f(1.0)),
        helper: None,
        options: None,
    },
];

const SERVICE_BROADCAST_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        key: "BroadcastType",
        label: "BroadcastType",
        kind: FieldKind::Select,
        required: Some(true),
        default: Some(json_const_s("Generic")),
        helper: None,
        options: Some(BROADCAST_TYPES),
    },
    FieldSpec {
        key: "Title",
        label: "Title",
        kind: FieldKind::String,
        required: None,
        default: None,
        helper: Some("required for Generic"),
        options: None,
    },
    FieldSpec {
        key: "Body",
        label: "Body",
        kind: FieldKind::Text,
        required: None,
        default: None,
        helper: Some("required for Generic"),
        options: None,
    },
    FieldSpec {
        key: "BroadcastDuration",
        label: "Duration (s)",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(30)),
        helper: None,
        options: None,
    },
];

const ONLY_PLAYER: &[FieldSpec] = &[FIELD_PLAYER];

const WATER_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "WaterAmount",
        label: "WaterAmount",
        kind: FieldKind::Int,
        required: None,
        default: Some(json_const_i(1_000_000)),
        helper: None,
        options: None,
    },
];

const AWARD_XP_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "Experience",
        label: "Experience",
        kind: FieldKind::Int,
        required: Some(true),
        default: Some(json_const_i(1000)),
        helper: Some("Generic player XP — the server ignores any track/category fields."),
        options: None,
    },
];

// AwardXPByEventTag was tried 2026-05-26 — server reports
// `Deserialized message has unknown Server Command 'AwardXPByEventTag'`.
// The binary has `ADuneCharacter::AwardXPByEventTag` but no MQ handler.

const SKILL_MODULE_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "Module",
        label: "Module",
        kind: FieldKind::String,
        required: Some(true),
        default: None,
        helper: Some("e.g. Swordmaster_T1"),
        options: None,
    },
    FieldSpec {
        key: "Level",
        label: "Level",
        kind: FieldKind::Int,
        required: Some(true),
        default: Some(json_const_i(1)),
        helper: None,
        options: None,
    },
];

const SKILL_POINTS_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "SkillPoints",
        label: "SkillPoints",
        kind: FieldKind::Int,
        required: Some(true),
        default: Some(json_const_i(0)),
        helper: None,
        options: None,
    },
];

const TELEPORT_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec {
        key: "X",
        label: "X",
        kind: FieldKind::Float,
        required: Some(true),
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Y",
        label: "Y",
        kind: FieldKind::Float,
        required: Some(true),
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Z",
        label: "Z",
        kind: FieldKind::Float,
        required: Some(true),
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "Yaw",
        label: "Yaw",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "CamPitch",
        label: "CamPitch",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "CamYaw",
        label: "CamYaw",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
    FieldSpec {
        key: "CamRoll",
        label: "CamRoll",
        kind: FieldKind::Float,
        required: None,
        default: None,
        helper: None,
        options: None,
    },
];

const SPAWN_VEHICLE_FIELDS: &[FieldSpec] = &[
    FIELD_PLAYER,
    FieldSpec { key: "ClassName", label: "Vehicle", kind: FieldKind::String, required: Some(true), default: None, helper: Some("DT_VehicleTemplates row key (e.g. Sandbike, Buggy)"), options: None },
    FieldSpec { key: "X", label: "X", kind: FieldKind::Float, required: Some(true), default: None, helper: None, options: None },
    FieldSpec { key: "Y", label: "Y", kind: FieldKind::Float, required: Some(true), default: None, helper: None, options: None },
    FieldSpec { key: "Z", label: "Z", kind: FieldKind::Float, required: Some(true), default: None, helper: None, options: None },
    FieldSpec { key: "Rotation", label: "Rotation", kind: FieldKind::Float, required: None, default: None, helper: None, options: None },
    FieldSpec { key: "TemplateName", label: "TemplateName", kind: FieldKind::String, required: Some(true), default: None, helper: Some("Template variant key from DT_VehicleTemplates (e.g. T6_Combat). Combobox above pre-fills the first valid one for the picked vehicle."), options: None },
    FieldSpec { key: "Persistent", label: "Persistent", kind: FieldKind::Float, required: None, default: Some(json_const_f(1.0)), helper: Some("0.0 = transient, 1.0 = persistent"), options: None },
    FieldSpec { key: "Faction", label: "Faction", kind: FieldKind::String, required: None, default: None, helper: Some("(blank = default)"), options: None },
];

pub static SPECS: &[CommandSpec] = &[
    CommandSpec {
        id: "AddItemToInventory",
        label: "Grant item",
        category: Category::Items,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Adds an item to the targeted player(s) inventory.",
        fields: ADD_ITEM_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "ServiceBroadcast",
        label: "Broadcast",
        category: Category::Broadcast,
        destructive: None,
        needs_player: false,
        allow_all_players: false,
        describe: "Server-wide broadcast (Generic) or ServerShutdown notice.",
        fields: SERVICE_BROADCAST_FIELDS,
        build: BuildKind::ServiceBroadcast,
    },
    CommandSpec {
        id: "KickPlayer",
        label: "Kick player",
        category: Category::Player,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Disconnects the targeted player(s).",
        fields: ONLY_PLAYER,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "CleanPlayerInventory",
        label: "Clean inventory",
        category: Category::Player,
        destructive: Some(true),
        needs_player: true,
        allow_all_players: true,
        describe: "Wipes the targeted player(s) inventory. Destructive.",
        fields: ONLY_PLAYER,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "ResetProgression",
        label: "Reset progression",
        category: Category::Player,
        destructive: Some(true),
        needs_player: true,
        allow_all_players: true,
        describe: "Wipes XP/skills/journey progress. Destructive.",
        fields: ONLY_PLAYER,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "UpdateAllWaterFillables",
        label: "Refill water",
        category: Category::Player,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Refills water in fillable containers carried by the player.",
        fields: WATER_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "AwardXP",
        label: "Award XP",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Adds generic player XP (server ignores any track/category fields).",
        fields: AWARD_XP_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SkillsSetModuleLevel",
        label: "Set skill module level",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Sets the level of a skill module for the player.",
        fields: SKILL_MODULE_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SkillsSetUnspentSkillPoints",
        label: "Set unspent skill points",
        category: Category::Progression,
        destructive: None,
        needs_player: true,
        allow_all_players: true,
        describe: "Sets the unspent skill points pool.",
        fields: SKILL_POINTS_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "TeleportTo",
        label: "Teleport (safe)",
        category: Category::Movement,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Teleports player to coordinates, snapping to safe location.",
        fields: TELEPORT_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "TeleportToExact",
        label: "Teleport (exact)",
        category: Category::Movement,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Teleports to exact coordinates with no safe-location snap.",
        fields: TELEPORT_FIELDS,
        build: BuildKind::Passthrough,
    },
    CommandSpec {
        id: "SpawnVehicleAt",
        label: "Spawn vehicle",
        category: Category::Movement,
        destructive: None,
        needs_player: true,
        allow_all_players: false,
        describe: "Spawns a vehicle at coordinates for the player.",
        fields: SPAWN_VEHICLE_FIELDS,
        build: BuildKind::Passthrough,
    },
    // Journey* commands removed 2026-05-26: published successfully and the
    // server-command handlers fire, but no observable state change in DB or
    // gameplay (live-tested). The `journey-nodes.json` data file remains in
    // case a working path resurfaces.
    //
    // ServerExec / CheatScript / RunLuaScriptFile removed earlier the same
    // day: published successfully but the seabass server's handler doesn't
    // execute them via the MQ path — only `LogPlayerController: COMMAND`
    // echoes, no DB or gameplay effect. Use the individual commands above.
];

const fn json_const_i(n: i64) -> serde_json::Value {
    // const fn body: we can't actually call serde_json::json! macro inside const yet;
    // workaround: rely on a Lazy at first use. Inline construction below.
    let _ = n;
    serde_json::Value::Null
}
const fn json_const_f(n: f64) -> serde_json::Value {
    let _ = n;
    serde_json::Value::Null
}
const fn json_const_s(s: &'static str) -> serde_json::Value {
    let _ = s;
    serde_json::Value::Null
}
