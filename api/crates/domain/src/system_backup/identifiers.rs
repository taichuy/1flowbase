use std::{error::Error, fmt};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupIdentifierError {
    field: &'static str,
}

impl BackupIdentifierError {
    fn new(field: &'static str) -> Self {
        Self { field }
    }
}

impl fmt::Display for BackupIdentifierError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid {}", self.field)
    }
}

impl Error for BackupIdentifierError {}

macro_rules! uuid_identifier {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            PartialOrd,
            Ord,
            Hash,
            Serialize,
            Deserialize,
            ToSchema,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

uuid_identifier!(BackupSetId);
uuid_identifier!(BackupJobId);
uuid_identifier!(RecoveryJobId);

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.len() <= 255
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/'))
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() >= 16 && value.len() <= 128 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

macro_rules! checked_string {
    ($name:ident, $field:literal, $validator:ident) => {
        #[derive(
            Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, ToSchema,
        )]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<&str> for $name {
            type Error = BackupIdentifierError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                $validator(value)
                    .then(|| Self(value.to_owned()))
                    .ok_or_else(|| BackupIdentifierError::new($field))
            }
        }

        impl TryFrom<String> for $name {
            type Error = BackupIdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::try_from(value.as_str())
            }
        }
    };
}

checked_string!(BackupComponentId, "backup component id", valid_identifier);
checked_string!(
    BackupSourceIdentity,
    "backup source identity",
    valid_identifier
);
checked_string!(ApplicationBuild, "application build", valid_identifier);
checked_string!(MigrationHead, "migration head", valid_identifier);
checked_string!(ContentDigest, "content digest", valid_fingerprint);
checked_string!(KeyFingerprint, "key fingerprint", valid_fingerprint);
