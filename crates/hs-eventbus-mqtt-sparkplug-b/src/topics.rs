pub fn dbirth_topic(group_id: &str, edge_node_id: &str, device_id: &str) -> String {
    format!(
        "spBv1.0/{}/DBIRTH/{}/{}",
        sanitize(group_id),
        sanitize(edge_node_id),
        sanitize(device_id)
    )
}

pub fn ddata_topic(group_id: &str, edge_node_id: &str, device_id: &str) -> String {
    format!(
        "spBv1.0/{}/DDATA/{}/{}",
        sanitize(group_id),
        sanitize(edge_node_id),
        sanitize(device_id)
    )
}

pub fn dcmd_topic(
    group_id: &str,
    edge_node_id: &str,
    device_id: &str,
) -> String {
    format!(
        "spBv1.0/{}/DCMD/{}/{}",
        sanitize(group_id),
        sanitize(edge_node_id),
        sanitize(device_id)
    )
}

pub fn state_topic(edge_node_id: &str) -> String {
    format!("spBv1.0/STATE/{}", sanitize(edge_node_id))
}

pub fn ncmd_topic(group_id: &str, edge_node_id: &str) -> String {
    format!(
        "spBv1.0/{}/NCMD/{}",
        sanitize(group_id),
        sanitize(edge_node_id)
    )
}

pub fn nbirth_topic(group_id: &str, edge_node_id: &str) -> String {
    format!(
        "spBv1.0/{}/NBIRTH/{}",
        sanitize(group_id),
        sanitize(edge_node_id)
    )
}

pub fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' => ch.to_ascii_lowercase(),
            _ => '_',
        })
        .collect()
}
