use enumflags2::{BitFlag, BitFlags};
use serde::{de::{DeserializeOwned, SeqAccess, Visitor}, ser::SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub fn serialize<S, T>(v: &BitFlags<T>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: BitFlag + Serialize,
{
    let mut seq = s.serialize_seq(Some(v.len()))?;
    for flag in v.iter()
    {
        seq.serialize_element(&flag)?;
    }
    seq.end()
}

pub fn deserialize<'de, D, T>(d: D) -> Result<BitFlags<T>, D::Error>
where
    D: Deserializer<'de>,
    T: BitFlag + DeserializeOwned,
{
    d.deserialize_seq(BitFlagsDeVisitor(std::marker::PhantomData))
}

struct BitFlagsDeVisitor<'de, F: BitFlag + Deserialize<'de>>(std::marker::PhantomData<&'de F>);
impl<'de, F: BitFlag + Deserialize<'de>> Visitor<'de> for BitFlagsDeVisitor<'de, F>
{
    type Value = BitFlags<F>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result
    {
        formatter.write_str("A sequence of enum variant strings")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error>
    {
        let mut flags = BitFlags::empty();
        while let Some(value) = seq.next_element::<F>()?
        {
            flags |= value
        }
        Ok(flags)
    }
}