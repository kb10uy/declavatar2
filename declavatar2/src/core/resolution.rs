#[derive(Debug, Clone)]
pub struct Unresolved<T>(pub T);

#[derive(Debug, Clone)]
pub struct Resolved<T, C> {
    pub value: T,
    pub context: C,
}
