use uuid::Uuid;
use wreq_util::Profile;

const COMPATIBLE_BROWSER_PROFILES: &[Profile] = &[
    Profile::Chrome132,
    Profile::Chrome133,
    Profile::Chrome134,
    Profile::Chrome135,
    Profile::Chrome136,
    Profile::Chrome137,
    Profile::Chrome138,
    Profile::Chrome139,
    Profile::Chrome140,
    Profile::Chrome141,
    Profile::Chrome142,
    Profile::Chrome143,
    Profile::Chrome144,
    Profile::Chrome145,
    Profile::Chrome146,
    Profile::Chrome147,
    Profile::Chrome148,
    Profile::Chrome149,
    Profile::Edge135,
    Profile::Edge136,
    Profile::Edge137,
    Profile::Edge138,
    Profile::Edge139,
    Profile::Edge140,
    Profile::Edge141,
    Profile::Edge142,
    Profile::Edge143,
    Profile::Edge144,
    Profile::Edge145,
    Profile::Edge146,
    Profile::Edge147,
    Profile::Edge148,
];

pub fn random() -> Profile {
    let index = (Uuid::new_v4().as_u128() % COMPATIBLE_BROWSER_PROFILES.len() as u128) as usize;
    COMPATIBLE_BROWSER_PROFILES[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use wreq_util::Emulation;

    #[test]
    fn when_default_emulation_is_used_then_all_candidates_should_have_the_current_alps_codepoint() {
        assert!(COMPATIBLE_BROWSER_PROFILES.len() > 1);
        for profile in COMPATIBLE_BROWSER_PROFILES {
            let emulation = wreq::IntoEmulation::into_emulation(
                Emulation::builder()
                    .profile(*profile)
                    .headers(false)
                    .build(),
            );

            assert_eq!(
                emulation
                    .tls_options
                    .map(|options| options.alps_use_new_codepoint),
                Some(true),
                "{profile:?}"
            );
            assert!(emulation.headers.is_empty(), "{profile:?}");
        }
    }
}
