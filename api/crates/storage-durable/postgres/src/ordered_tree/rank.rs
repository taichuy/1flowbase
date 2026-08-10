use std::{cmp::Ordering, error::Error, fmt};

const ALPHABET: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const MID_DIGIT: u8 = b'U';
const REBALANCE_LENGTH_THRESHOLD: usize = 32;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FractionalRank(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RankError {
    InvalidEncoding,
    InvalidBounds,
    RebalanceCapacityExceeded,
}

impl fmt::Display for RankError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidEncoding => formatter.write_str("invalid fractional-rank encoding"),
            Self::InvalidBounds => formatter.write_str("fractional-rank bounds are not ordered"),
            Self::RebalanceCapacityExceeded => {
                formatter.write_str("fractional-rank rebalance capacity exceeded")
            }
        }
    }
}

impl Error for RankError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankAllocation {
    pub rank: FractionalRank,
    pub rebalance_recommended: bool,
}

impl FractionalRank {
    pub fn parse(value: impl Into<String>) -> Result<Self, RankError> {
        let value = value.into();
        if value.is_empty()
            || value.as_bytes().last() == Some(&ALPHABET[0])
            || !value.bytes().all(|byte| ALPHABET.contains(&byte))
        {
            return Err(RankError::InvalidEncoding);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn between(
    left: Option<&FractionalRank>,
    right: Option<&FractionalRank>,
) -> Result<RankAllocation, RankError> {
    if matches!((left, right), (Some(left), Some(right)) if left >= right) {
        return Err(RankError::InvalidBounds);
    }

    let encoded = match (left, right) {
        (None, None) => MID_DIGIT.to_string(),
        (Some(left), None) => format!("{}{MID_DIGIT}", left.as_str()),
        (None, Some(right)) => before(right.as_str())?,
        (Some(left), Some(right)) => between_encoded(left.as_str(), right.as_str())?,
    };
    let rank = FractionalRank::parse(encoded)?;
    Ok(RankAllocation {
        rebalance_recommended: rank.as_str().len() > REBALANCE_LENGTH_THRESHOLD,
        rank,
    })
}

pub fn rebalance(count: usize) -> Result<Vec<FractionalRank>, RankError> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let base = ALPHABET.len() as u128;
    let minimum_capacity = (count as u128 + 1)
        .checked_mul(2)
        .ok_or(RankError::RebalanceCapacityExceeded)?;
    let mut width = 1usize;
    let mut capacity = base;
    while capacity <= minimum_capacity {
        capacity = capacity
            .checked_mul(base)
            .ok_or(RankError::RebalanceCapacityExceeded)?;
        width += 1;
    }

    let step = capacity / (count as u128 + 1);
    (1..=count)
        .map(|position| {
            let mut value = step * position as u128;
            if value.is_multiple_of(base) {
                value += 1;
            }
            FractionalRank::parse(encode_fixed(value, width))
        })
        .collect()
}

fn between_encoded(left: &str, right: &str) -> Result<String, RankError> {
    let prefix_len = left
        .bytes()
        .zip(right.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    if prefix_len == right.len() {
        return Err(RankError::InvalidBounds);
    }
    if prefix_len == left.len() {
        let mut rank = left.to_owned();
        rank.push_str(&before(&right[prefix_len..])?);
        return Ok(rank);
    }

    let left_digit = digit_index(left.as_bytes()[prefix_len])?;
    let right_digit = digit_index(right.as_bytes()[prefix_len])?;
    if right_digit - left_digit > 1 {
        let mut rank = left[..prefix_len].to_owned();
        rank.push(ALPHABET[(left_digit + right_digit) / 2] as char);
        return Ok(rank);
    }

    Ok(format!("{left}{MID_DIGIT}"))
}

fn before(right: &str) -> Result<String, RankError> {
    let first = *right.as_bytes().first().ok_or(RankError::InvalidBounds)?;
    let right_digit = digit_index(first)?;
    if right_digit == 0 {
        let mut rank = String::from("0");
        rank.push_str(&before(&right[1..])?);
        return Ok(rank);
    }

    let midpoint = right_digit / 2;
    let mut rank = String::from(ALPHABET[midpoint] as char);
    if midpoint == 0 {
        rank.push(MID_DIGIT as char);
    }
    Ok(rank)
}

fn digit_index(digit: u8) -> Result<usize, RankError> {
    ALPHABET
        .binary_search_by(|candidate| {
            if *candidate < digit {
                Ordering::Less
            } else if *candidate > digit {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .map_err(|_| RankError::InvalidEncoding)
}

fn encode_fixed(mut value: u128, width: usize) -> String {
    let mut encoded = vec![ALPHABET[0]; width];
    for digit in encoded.iter_mut().rev() {
        *digit = ALPHABET[(value % ALPHABET.len() as u128) as usize];
        value /= ALPHABET.len() as u128;
    }
    String::from_utf8(encoded).expect("fractional-rank alphabet is valid UTF-8")
}
