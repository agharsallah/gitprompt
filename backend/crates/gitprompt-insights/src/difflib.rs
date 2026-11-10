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

    /// The longest matching block in `a[alo..ahi]` and `b[blo..bhi]`; of several, the one that
    /// starts earliest in `a`, then earliest in `b`.
    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> Block {
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0);
        // j2len[j]: length of the longest match ending with a[i - 1] and b[j].
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut next: HashMap<usize, usize> = HashMap::new();
            for &j in self.b2j.get(&self.a[i]).map_or(&[][..], Vec::as_slice) {
                if j < blo {
                    continue;
                }
                if j >= bhi {
                    break;
                }
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
        let mut queue = vec![(0, la, 0, lb)];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let found = self.find_longest_match(alo, ahi, blo, bhi);
            if found.size > 0 {
                blocks.push(found);
                if alo < found.i && blo < found.j {
                    queue.push((alo, found.i, blo, found.j));
                }
                if found.i + found.size < ahi && found.j + found.size < bhi {
                    queue.push((found.i + found.size, ahi, found.j + found.size, bhi));
                }
            }
        }
        blocks.sort();
        let mut merged: Vec<Block> = Vec::new();
        let mut current = Block { i: 0, j: 0, size: 0 };
        for block in blocks {
            if current.i + current.size == block.i && current.j + current.size == block.j {
                current.size += block.size;
            } else {
                if current.size > 0 {
                    merged.push(current);
                }
                current = block;
            }
        }
        if current.size > 0 {
            merged.push(current);
        }
        merged.push(Block { i: la, j: lb, size: 0 });
        merged
    }
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
        let (a, b): (Vec<char>, Vec<char>) = ("qabxcd".chars().collect(), "abycdf".chars().collect());
        use Tag::*;
        assert_eq!(
            opcodes(&a, &b),
            vec![(Delete, 0, 1, 0, 0), (Equal, 1, 3, 0, 2), (Replace, 3, 4, 2, 3), (Equal, 4, 6, 3, 5), (Insert, 6, 6, 5, 6)]
        );
    }
}
