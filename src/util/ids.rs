use uuid::Uuid;

/// Generate a new random UUID v4.
#[allow(dead_code)]
pub fn new_id() -> Uuid {
    Uuid::new_v4()
}
