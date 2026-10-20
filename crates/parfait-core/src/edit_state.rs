//! Runtime logical state identity; deliberately absent from Document serialization.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditStateId([u8; 16]);

impl EditStateId {
    pub fn try_new() -> Result<Self, getrandom::Error> {
        Self::generate(getrandom::fill)
    }

    fn generate(
        fill: impl FnOnce(&mut [u8]) -> Result<(), getrandom::Error>,
    ) -> Result<Self, getrandom::Error> {
        let mut bytes = [0; 16];
        fill(&mut bytes)?;
        Ok(Self(bytes))
    }

    pub fn bytes(self) -> [u8; 16] {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multi_document_identity_generation_fails_without_fallback() {
        assert!(
            EditStateId::generate(|_| Err(getrandom::Error::UNSUPPORTED))
                .is_err()
        );
        assert_ne!(
            EditStateId::try_new().unwrap(),
            EditStateId::try_new().unwrap()
        );
    }
}
