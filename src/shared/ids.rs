use uuid::Uuid;

/// Generate a new random UUID v4.
pub fn new_id() -> Uuid {
    Uuid::new_v4()
}
