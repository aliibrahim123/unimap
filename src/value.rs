#[derive(Debug, Clone, PartialEq, Eq, Copy, Default)]
pub struct Id(pub usize);
impl Into<usize> for Id {
	fn into(self) -> usize {
		self.0
	}
}
