//! Python's `difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes()`, ported line by line
//! so word diffs stay identical to the Python dashboard's. Without `isjunk` and with `autojunk` off
//! no element of `b` is junk or popular, so those branches of the original are left out.

use std::collections::HashMap;
use std::hash::Hash;

/// What a range of `a` became in `b`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tag {
    Equal,
    Replace,
    Delete,
    Insert,
}

/// `(tag, i1, i2, j1, j2)`: `a[i1..i2]` became `b[j1..j2]`.
pub(crate) type Opcode = (Tag, usize, usize, usize, usize);

/// A run of equal elements: `a[i..i + size] == b[j..j + size]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Block {
    i: usize,
    j: usize,
    size: usize,
}

struct Matcher<'s, T> {
    a: &'s [T],
    b: &'s [T],
    /// Every index of each element of `b`, ascending.
    b2j: HashMap<&'s T, Vec<usize>>,
}

impl<'s, T: Eq + Hash> Matcher<'s, T> {
    fn new(a: &'s [T], b: &'s [T]) -> Self {
        let mut b2j: HashMap<&T, Vec<usize>> = HashMap::new();
        for (j, element) in b.iter().enumerate() {
            b2j.entry(element).or_default().push(j);
        }
        Matcher { a, b, b2j }
    }

    /// Every index of `element` in `b`, ascending.
    fn b_indices(&self, element: &T) -> impl Iterator<Item = usize> + '_ {
        self.b2j.get(element).map_or(&[][..], Vec::as_slice).iter().copied()
    }

    /// The longest matching block in `a[alo..ahi]` and `b[blo..bhi]`; of several, the one that
    /// starts earliest in `a`, then earliest in `b`.
    #[expect(clippy::indexing_slicing, reason = "a port of Python's difflib: i < ahi <= len(a), and each loop checks its bounds first")]
    #[expect(
        clippy::excessive_nesting,
        reason = "difflib's find_longest_match kept line for line (rows of a, then runs in b) so the port can be checked against the Python"
    )]
    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> Block {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0);
        // j2len[j]: length of the longest match ending with a[i - 1] and b[j].
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut next: HashMap<usize, usize> = HashMap::new();
            // Indices are ascending: skip those before `blo`, stop at `bhi`.
            for j in self.b_indices(&self.a[i]).skip_while(|&j| j < blo).take_while(|&j| j < bhi) {
                let k = j.checked_sub(1).and_then(|prev| j2len.get(&prev)).copied().unwrap_or(0) + 1;
                next.insert(j, k);
                if k > bestsize {
                    (besti, bestj, bestsize) = (i + 1 - k, j + 1 - k, k);
                }
            }
            j2len = next;
        }
        // Extend with equal neighbours (only ever needed for junk elements in the original, kept so
        // the port stays exact).
        while besti > alo && bestj > blo && self.a[besti - 1] == self.b[bestj - 1] {
            (besti, bestj, bestsize) = (besti - 1, bestj - 1, bestsize + 1);
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && self.a[besti + bestsize] == self.b[bestj + bestsize] {
            bestsize += 1;
        }
        Block { i: besti, j: bestj, size: bestsize }
    }

    /// Matching blocks in order, adjacent ones merged, ending with the `(len a, len b, 0)` sentinel.
    fn matching_blocks(&self) -> Vec<Block> {
        let (la, lb) = (self.a.len(), self.b.len());
        let mut blocks = self.longest_matches(la, lb);
        blocks.sort();
        let mut merged = merge_adjacent(blocks);
        merged.push(Block { i: la, j: lb, size: 0 });
        merged
    }

    /// The longest match of `a[..la]` and `b[..lb]`, then recursively of what lies before and after
    /// it on both sides (unordered).
    fn longest_matches(&self, la: usize, lb: usize) -> Vec<Block> {
        let mut queue = vec![(0, la, 0, lb)];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let found = self.find_longest_match(alo, ahi, blo, bhi);
            if found.size == 0 {
                continue;
            }
            blocks.push(found);
            if alo < found.i && blo < found.j {
                queue.push((alo, found.i, blo, found.j));
            }
            if found.i + found.size < ahi && found.j + found.size < bhi {
                queue.push((found.i + found.size, ahi, found.j + found.size, bhi));
            }
        }
        blocks
    }
}

/// Sorted blocks with each one that continues the previous (in both `a` and `b`) joined to it.
fn merge_adjacent(blocks: Vec<Block>) -> Vec<Block> {
    let mut merged: Vec<Block> = Vec::new();
    for block in blocks {
        match merged.last_mut() {
            Some(last) if last.i + last.size == block.i && last.j + last.size == block.j => last.size += block.size,
            _ => merged.push(block),
        }
    }
    merged
}

/// How to turn `a` into `b`.
pub(crate) fn opcodes<T: Eq + Hash>(a: &[T], b: &[T]) -> Vec<Opcode> {
    let (mut i, mut j) = (0, 0);
    let mut out = Vec::new();
    for block in Matcher::new(a, b).matching_blocks() {
        let tag = match (i < block.i, j < block.j) {
            (true, true) => Some(Tag::Replace),
            (true, false) => Some(Tag::Delete),
            (false, true) => Some(Tag::Insert),
            (false, false) => None,
        };
        if let Some(tag) = tag {
            out.push((tag, i, block.i, j, block.j));
        }
        (i, j) = (block.i + block.size, block.j + block.size);
        if block.size > 0 {
            out.push((Tag::Equal, block.i, i, block.j, j));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `SequenceMatcher(None, "qabxcd", "abycdf", autojunk=False).get_opcodes()` in Python.
    #[test]
    fn opcodes_match_python() {
        use Tag::*;
        let (a, b): (Vec<char>, Vec<char>) = ("qabxcd".chars().collect(), "abycdf".chars().collect());
        assert_eq!(
            opcodes(&a, &b),
            vec![(Delete, 0, 1, 0, 0), (Equal, 1, 3, 0, 2), (Replace, 3, 4, 2, 3), (Equal, 4, 6, 3, 5), (Insert, 6, 6, 5, 6)]
        );
    }
}
