use crate::AuthError;

/// 6자리 일회용 인증 code. `Debug` 는 숫자를 가리며, `Clone` 가능하다.
#[derive(Clone, PartialEq, Eq)]
pub struct OneTimeCode(String);

impl std::fmt::Debug for OneTimeCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "OneTimeCode(******)")
    }
}

impl OneTimeCode {
    /// 000000..999999 균등 분포. `getrandom::fill` + 거부 표본.
    pub fn generate() -> OneTimeCode {
        const LIMIT: u32 = (u32::MAX / 1_000_000) * 1_000_000;
        loop {
            let mut buf = [0u8; 4];
            getrandom::fill(&mut buf).expect("system RNG must succeed");
            let n = u32::from_be_bytes(buf);
            if n < LIMIT {
                let value = n % 1_000_000;
                return OneTimeCode(format!("{value:06}"));
            }
        }
    }

    /// 공백을 제거한 뒤 정확히 숫자 6개인지 확인한다.
    pub fn parse(input: &str) -> Result<OneTimeCode, AuthError> {
        let digits: String = input.chars().filter(|c| !c.is_whitespace()).collect();
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err(AuthError::Malformed("one-time code"));
        }
        Ok(OneTimeCode(digits))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn generate_produces_six_digit_codes_with_variety() {
        let codes: Vec<OneTimeCode> = (0..200).map(|_| OneTimeCode::generate()).collect();
        for c in &codes {
            assert_eq!(c.as_str().len(), 6);
            assert!(c.as_str().bytes().all(|b| b.is_ascii_digit()));
        }
        let unique: HashSet<&str> = codes.iter().map(|c| c.as_str()).collect();
        assert!(unique.len() >= 2);
    }

    #[test]
    fn parse_strips_whitespace() {
        assert_eq!(OneTimeCode::parse("123 456").unwrap().as_str(), "123456");
    }

    #[test]
    fn parse_rejects_wrong_shape() {
        assert!(matches!(
            OneTimeCode::parse("12345"),
            Err(AuthError::Malformed(_))
        ));
        assert!(matches!(
            OneTimeCode::parse("1234567"),
            Err(AuthError::Malformed(_))
        ));
        assert!(matches!(
            OneTimeCode::parse("12a456"),
            Err(AuthError::Malformed(_))
        ));
    }

    #[test]
    fn debug_masks_digits() {
        let code = OneTimeCode::parse("123456").unwrap();
        let debug = format!("{code:?}");
        assert!(!debug.contains("123456"));
        assert_eq!(debug, "OneTimeCode(******)");
    }
}
