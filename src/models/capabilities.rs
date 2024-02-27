bitflags::bitflags! {
    #[repr(transparent)]
    #[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct B2Capability: u32 {
        const LIST_KEYS = 1 << 0;
        const WRITE_KEYS = 1 << 1;
        const DELETE_KEYS = 1 << 2;
        const LIST_ALL_BUCKET_NAMES = 1 << 3;
        const LIST_BUCKETS = 1 << 4;
        const READ_BUCKETS = 1 << 5;
        const WRITE_BUCKETS = 1 << 6;
        const DELETE_BUCKETS = 1 << 7;
        const READ_BUCKET_RETENTIONS = 1 << 8;
        const WRITE_BUCKET_RETENTIONS = 1 << 9;
        const READ_BUCKET_ENCRYPTION = 1 << 10;
        const WRITE_BUCKET_ENCRYPTION = 1 << 11;
        const LIST_FILES = 1 << 12;
        const READ_FILES = 1 << 13;
        const SHARE_FILES = 1 << 14;
        const WRITE_FILES = 1 << 15;
        const DELETE_FILES = 1 << 16;
        const READ_FILE_LEGAL_HOLDS = 1 << 17;
        const WRITE_FILE_LEGAL_HOLDS = 1 << 18;
        const READ_FILE_RETENTIONS = 1 << 19;
        const WRITE_FILE_RETENTIONS = 1 << 20;
        const BYPASS_GOVERNANCE = 1 << 21;
        const READ_BUCKET_REPLICATIONS = 1 << 22;
        const WRITE_BUCKET_REPLICATIONS = 1 << 23;

        /// When creating an application key restricted to a bucket, these are the capabilities that can be used.
        const ALLOWED_CAPABILITIES_IN_BUCKET_KEY = Self::LIST_ALL_BUCKET_NAMES.bits()
            | Self::LIST_BUCKETS.bits()
            | Self::READ_BUCKETS.bits()
            | Self::READ_BUCKET_ENCRYPTION.bits()
            | Self::WRITE_BUCKET_ENCRYPTION.bits()
            | Self::READ_BUCKET_RETENTIONS.bits()
            | Self::WRITE_BUCKET_RETENTIONS.bits()
            | Self::LIST_FILES.bits()
            | Self::READ_FILES.bits()
            | Self::SHARE_FILES.bits()
            | Self::WRITE_FILES.bits()
            | Self::DELETE_FILES.bits()
            | Self::READ_FILE_LEGAL_HOLDS.bits()
            | Self::WRITE_FILE_LEGAL_HOLDS.bits()
            | Self::READ_FILE_RETENTIONS.bits()
            | Self::WRITE_FILE_RETENTIONS.bits()
            | Self::BYPASS_GOVERNANCE.bits();
    }
}

impl B2Capability {
    /// Takes the union of two sets of capabilities only if the condition is true.
    #[rustfmt::skip]
    pub const fn cond_union(self, cond: bool, other: Self) -> Self {
        if cond { self.union(other) } else { self }
    }

    pub const ALL_CAPABILITIES_AND_NAMES: [(B2Capability, &'static str); 24] = [
        (B2Capability::LIST_KEYS, "listKeys"),
        (B2Capability::WRITE_KEYS, "writeKeys"),
        (B2Capability::DELETE_KEYS, "deleteKeys"),
        (B2Capability::LIST_ALL_BUCKET_NAMES, "listAllBucketNames"),
        (B2Capability::LIST_BUCKETS, "listBuckets"),
        (B2Capability::READ_BUCKETS, "readBuckets"),
        (B2Capability::WRITE_BUCKETS, "writeBuckets"),
        (B2Capability::DELETE_BUCKETS, "deleteBuckets"),
        (B2Capability::READ_BUCKET_RETENTIONS, "readBucketRetentions"),
        (B2Capability::WRITE_BUCKET_RETENTIONS, "writeBucketRetentions"),
        (B2Capability::READ_BUCKET_ENCRYPTION, "readBucketEncryption"),
        (B2Capability::WRITE_BUCKET_ENCRYPTION, "writeBucketEncryption"),
        (B2Capability::LIST_FILES, "listFiles"),
        (B2Capability::READ_FILES, "readFiles"),
        (B2Capability::SHARE_FILES, "shareFiles"),
        (B2Capability::WRITE_FILES, "writeFiles"),
        (B2Capability::DELETE_FILES, "deleteFiles"),
        (B2Capability::READ_FILE_LEGAL_HOLDS, "readFileLegalHolds"),
        (B2Capability::WRITE_FILE_LEGAL_HOLDS, "writeFileLegalHolds"),
        (B2Capability::READ_FILE_RETENTIONS, "readFileRetentions"),
        (B2Capability::WRITE_FILE_RETENTIONS, "writeFileRetentions"),
        (B2Capability::BYPASS_GOVERNANCE, "bypassGovernance"),
        (B2Capability::READ_BUCKET_REPLICATIONS, "readBucketReplications"),
        (B2Capability::WRITE_BUCKET_REPLICATIONS, "writeBucketReplications"),
    ];

    const ALL_NAMES: [&'static str; 24] = [
        "listKeys",
        "writeKeys",
        "deleteKeys",
        "listAllBucketNames",
        "listBuckets",
        "readBuckets",
        "writeBuckets",
        "deleteBuckets",
        "readBucketRetentions",
        "writeBucketRetentions",
        "readBucketEncryption",
        "writeBucketEncryption",
        "listFiles",
        "readFiles",
        "shareFiles",
        "writeFiles",
        "deleteFiles",
        "readFileLegalHolds",
        "writeFileLegalHolds",
        "readFileRetentions",
        "writeFileRetentions",
        "bypassGovernance",
        "readBucketReplications",
        "writeBucketReplications",
    ];
}

/// A set of B2 capabilities that (de)serializes as a list of strings.
#[repr(transparent)]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct B2CapabilitiesStringSet {
    caps: B2Capability,
}

impl std::ops::Deref for B2CapabilitiesStringSet {
    type Target = B2Capability;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.caps
    }
}

impl std::ops::DerefMut for B2CapabilitiesStringSet {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.caps
    }
}

impl From<B2Capability> for B2CapabilitiesStringSet {
    #[inline(always)]
    fn from(caps: B2Capability) -> Self {
        B2CapabilitiesStringSet { caps }
    }
}

impl From<B2CapabilitiesStringSet> for B2Capability {
    #[inline(always)]
    fn from(caps: B2CapabilitiesStringSet) -> Self {
        caps.caps
    }
}

use serde::de::{Deserialize, Deserializer, Error, Visitor};
use serde::ser::{Serialize, SerializeSeq, Serializer};

impl Serialize for B2CapabilitiesStringSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.bits().count_ones() as usize))?;
        for (cap, name) in B2Capability::ALL_CAPABILITIES_AND_NAMES.iter() {
            if self.contains(*cap) {
                seq.serialize_element(name)?;
            }
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for B2CapabilitiesStringSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        return deserializer.deserialize_seq(CapSetVisitor);

        struct CapSetVisitor;
        impl<'de> Visitor<'de> for CapSetVisitor {
            type Value = B2CapabilitiesStringSet;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a list of strings representing B2 capabilities")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut caps = B2Capability::empty();

                while let Some(name) = seq.next_element::<&'de str>()? {
                    match B2Capability::ALL_CAPABILITIES_AND_NAMES
                        .iter()
                        .find(|(_, n)| n.eq_ignore_ascii_case(name))
                    {
                        Some((cap, _)) => caps |= *cap,
                        None => return Err(A::Error::unknown_variant(name, &B2Capability::ALL_NAMES)),
                    }
                }

                Ok(B2CapabilitiesStringSet { caps })
            }
        }
    }
}
