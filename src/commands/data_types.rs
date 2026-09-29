use std::collections::{BTreeMap, HashSet};
use std::collections::{HashMap, VecDeque};
use std::ops::{Deref, DerefMut};
use std::{u8, u64};
use std::rc::Rc;

use crate::exceptions::CustomError;

// A B

// Wrapper for score value f64 needed
// because BTreeMap requires the key to implement Ord
// while f64 does not implement Ord (due to NaN value), only PartialOrd
#[derive(PartialEq, Debug)]
pub struct Score(f64);  // wrapper for f64 used in ZSet

impl Eq for Score {}    // This is no fn trait, for Score(_) == Score(_)

// PartialOrd and Ord must agree, or non-canonical implementations error
// thus partial_cmp(a, b) must == Some(cmp(a, b))
impl PartialOrd for Score {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// f64 implemented PartialOrd, then partial_cmp
impl Ord for Score {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap()
    }
}

// SortedSet neet a separate implementation
// as both member is unique, and score need to be SortedSet
#[derive(Debug)]
pub struct SortedSet<> {
    pub members: HashMap<Rc<String>, f64>,      // No need Rc<f64>, not saving any mem
    pub scores: BTreeMap<(Score, Rc<String>), ()>,
}

impl SortedSet {
    pub fn new() -> Self {
        Self { members: HashMap::new(), scores: BTreeMap::new() }
    }

    // pub fn members(&self) -> &HashMap<Rc<String>, f64> {
    //     &self.members
    // }

    pub fn add(&mut self, score: f64, member: String) -> i64 {
        // Add or update score of a member
        // Return the nbr of newly added member
        let mut new_mem_count = 1;
        let member_rc = Rc::from(member);

        if self.members.contains_key(&member_rc) {
            // Remove score
            let _ = self.scores.remove(&(Score(score), member_rc.clone())); 
            new_mem_count = 0;
        };

        // Add new
        self.members.insert(member_rc.clone(), score);
        self.scores.insert((Score(score), member_rc), ());
        new_mem_count
    }

    pub fn rank_by_score(&self, member: &Rc<String>) -> Option<i64> {
        let score = self.members.get(member)?;
        let target_score = (Score(*score), member.clone());
        let rank = self.scores.range(..target_score).count();
        Some(rank as i64)
    }

    pub fn get_members(&self, start_idx: i64, end_idx: i64) -> Vec<String> {
        let member_count = self.scores.len() as i64;

        // Handle negative index
        let start_idx = if start_idx < 0 {
            (member_count + start_idx).max(0)
        } else {
            start_idx
        };

        let end_idx = if end_idx < 0 {
            (member_count + end_idx).max(0)
        } else {
            end_idx
        };

        if end_idx < start_idx { return Vec::new()};

        self.scores.iter()
            .skip(start_idx as usize)
            .take((end_idx - start_idx + 1) as usize)
            .map(|((_, member_rc), ())| (**member_rc).clone())
            .collect()
    }

    pub fn get_score(&self, member: &Rc<String>) -> Option<f64> {
        self.members.get(member).copied()
    }

    pub fn remove(&mut self, member: String) -> i64 {
        let member_rc = Rc::from(member);
        if let Some(score) =  self.members.remove(&member_rc) {
            self.scores.remove(&(Score(score), member_rc));
            1
        } else {
            0
        }
    }

    pub fn len(&self) -> usize {
        self.members.len()
    }
    
}

// --------- For Bitmap
pub enum BitValue {
    Zero,
    One,
}

impl TryFrom<u8> for BitValue {
    type Error = CustomError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == 1 {
            Ok(Self::One)
        } else if value == 0 {
            Ok(Self::Zero)
        } else {
            Err(CustomError::ParseError("Input must be either 0 or 1".to_string()))
        }
    }
}

/// Unit of BITCOUNT range, BYTE by default
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BitUnit {
    Byte,
    Bit,
}

/// Inclusive BITCOUNT range, negative index counts from the end
#[derive(Debug, PartialEq, Clone, Copy)]
pub struct BitRange {
    pub start: i64,
    pub end: i64,
    pub unit: BitUnit,
}

/// Trait for bitmap operations
pub struct BitOps;

impl BitOps {
    // fn len(&self) -> usize;

    /// Checks whether bit at `offset` is set.
    /// Out-of-index index return `false`, so no panic.
    /// As bitmap is block of size of T, get/set fails if offset > total size
    pub fn get(map: &[u8], offset: usize) -> Result<u8, CustomError>
    {
        let block_index = offset / 8;
        let block = map.get(block_index).ok_or(CustomError::OutOfIndex)?;
        // Redis bit order: offset 0 is the most significant bit of the first byte
        let block_offset = 7 - offset % 8;
        let mask = 1u8 << block_offset;
        if  *block & mask != 0 {
            Ok(1)
        } else {
            Ok(0)
        }
    }

    pub fn set(map: &mut [u8], offset: usize, value: &BitValue) -> Result<(), CustomError>
    {
        let block_index = offset / 8;
        let block = map.get_mut(block_index).ok_or(CustomError::OutOfIndex)?;
        let block_offset = 7 - offset % 8;

        match value {
            BitValue::One => {
                let mask = 1u8 << block_offset;
                *block |= mask;
            }
            BitValue::Zero => {
                let mask = (1u8 << block_offset) ^ u8::MAX;
                *block &= mask;
            }
        };
        Ok(())
    }

    /// Counts set bits, over the whole map or within an inclusive `range`
    pub fn bit_count(map: &[u8], range: Option<BitRange>) -> usize {
        let Some(range) = range else {
            return map.iter().map(|b| b.count_ones() as usize).sum();
        };

        let len = match range.unit {
            BitUnit::Byte => map.len() as i64,
            BitUnit::Bit => map.len() as i64 * 8,
        };

        // Handle negative index, then clamp to [0, len - 1]
        let start = if range.start < 0 { (len + range.start).max(0) } else { range.start };
        let end = if range.end < 0 { (len + range.end).max(0) } else { range.end };
        let end = end.min(len - 1);
        if len == 0 || start > end { return 0 };

        let (start, end) = (start as usize, end as usize);
        match range.unit {
            BitUnit::Byte => map[start..=end].iter().map(|b| b.count_ones() as usize).sum(),
            BitUnit::Bit => (start..=end)
                .filter(|&i| Self::get(map, i).unwrap_or(0) > 0)
                .count(),
        }
    }

    /// Byte-wise AND, the shorter map is padded with zero bytes
    pub fn and(map1: &[u8], map2: &[u8]) -> Vec<u8> {
        Self::combine(map1, map2, |a, b| a & b)
    }

    /// Byte-wise OR, the shorter map is padded with zero bytes
    pub fn or(map1: &[u8], map2: &[u8]) -> Vec<u8> {
        Self::combine(map1, map2, |a, b| a | b)
    }

    fn combine(map1: &[u8], map2: &[u8], op: impl Fn(u8, u8) -> u8) -> Vec<u8> {
        let max_len = map1.len().max(map2.len());
        (0..max_len)
            .map(|i| op(*map1.get(i).unwrap_or(&0u8), *map2.get(i).unwrap_or(&0u8)))
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Bitmap(Vec<u8>);

impl Bitmap {
    pub fn new(length: usize) -> Self {
        let blocks = length.div_ceil(8);
        let map: Vec<u8> = if length > 0 {
            (0..blocks).into_iter().map(|_| 0u8).collect()
        } else {
            Vec::new()
        };
        Self(map)
    }

    /// Extend the bitmap to accomodate `offset`
    pub fn extend(&mut self, offset: usize) {
        let extend = (offset + 1).div_ceil(8).saturating_sub(self.0.len());
        if extend > 0 {
            let new_map: Vec<u8> = (0..extend).map(|_| 0u8).collect();
            self.0.extend(new_map);
        };
    }
}

impl From<Vec<u8>> for Bitmap {
    fn from(map: Vec<u8>) -> Self {
        Self(map)
    }
}

impl Deref for Bitmap {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        self.0.as_slice() 
    }
}

impl DerefMut for Bitmap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut_slice() 
    }
}

// Stored value types for CmdHandler
#[derive(Debug)]
pub enum StoreValue {
    Str(String),
    List(VecDeque<String>),
    Set(HashSet<String>),
    ZSet(SortedSet),
    Hash(HashMap<String, String>),
    // timestamp id - (timestamp, order, Vec of String)
    Stream(BTreeMap<(u64, u64), Vec<String>>),
    VectorSet(String),
    Bitmap(Bitmap),
    None
}

impl StoreValue {
    pub const fn get_type(&self) -> &str {
        match self {
            Self::Str(_) => "string", 
            Self::List(_) => "list",
            Self::Set(_) => "set",
            Self::ZSet(_) => "zset",
            Self::Hash(_) => "hash",
            Self::Stream(_) => "stream",
            Self::VectorSet(_) => "vectorset",
            Self::Bitmap(_) => "bitmap",
            _ => "none",
        }
    }
}

#[derive(Debug)]
pub struct StoreItem {
    pub value: StoreValue,
    pub expired_at: Option<u64>
}

impl StoreItem {
    pub fn new(value: StoreValue, expired_at: Option<u64>) -> Self {
        Self { value, expired_at }
    }
}
