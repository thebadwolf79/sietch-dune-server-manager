pub fn friendly_map_name(map: &str, fallback_name: &str) -> String {
    let normalized = map.to_ascii_lowercase();
    if normalized == "survival_1" || fallback_name.contains("survival-1") {
        return "Hagga Basin".to_string();
    }
    if normalized == "overmap" || fallback_name.contains("overmap") {
        return "Overmap".to_string();
    }
    if normalized.contains("deepdesert") || fallback_name.contains("deepdesert") {
        return "Deep Desert".to_string();
    }
    if fallback_name.contains("sh-arrakeen") {
        return "Social Hub: Arrakeen".to_string();
    }
    if fallback_name.contains("sh-harkovillage") {
        return "Social Hub: Harko Village".to_string();
    }
    if !map.is_empty() {
        return map.replace('_', " ");
    }
    "Game Server".to_string()
}

pub fn serverset_log_key(name: &str, map: &str) -> String {
    let combined = format!("{name} {map}").to_ascii_lowercase();
    if map.eq_ignore_ascii_case("Survival_1") || combined.contains("survival-1") {
        return "map-survival-1".to_string();
    }
    if map.eq_ignore_ascii_case("Overmap") || combined.contains("overmap") {
        return "map-overmap".to_string();
    }
    if combined.contains("deepdesert") || combined.contains("deep-desert") {
        return "map-deepdesert".to_string();
    }
    if combined.contains("sh-arrakeen") {
        return "map-social-arrakeen".to_string();
    }
    if combined.contains("sh-harkovillage") {
        return "map-social-harkovillage".to_string();
    }
    format!("map-{}", sanitize_component_key(map))
}

fn sanitize_component_key(value: &str) -> String {
    let key = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if key.is_empty() {
        "unknown".to_string()
    } else {
        key
    }
}
