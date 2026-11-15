use std::collections::{HashMap, VecDeque};

use unicode_normalization::{UnicodeNormalization, char::canonical_combining_class};
use uuid::Uuid;

use super::{DocumentMention, RegisteredStock};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ChunkText<'a> {
    pub id: Uuid,
    pub start_offset: i32,
    pub body: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InvalidOffset;

#[derive(Debug, Clone, Copy)]
struct SourceSpan {
    start: usize,
    end: usize,
}

struct StockPattern {
    stock_id: String,
    characters: Vec<char>,
}

#[derive(Default)]
struct TrieNode {
    transitions: HashMap<char, usize>,
    failure: usize,
    outputs: Vec<usize>,
}

struct StockMatcher {
    patterns: Vec<StockPattern>,
    nodes: Vec<TrieNode>,
}

pub(super) fn find_mentions(
    chunks: &[ChunkText<'_>],
    stocks: &[RegisteredStock],
) -> Result<Vec<DocumentMention>, InvalidOffset> {
    let matcher = StockMatcher::new(stocks);
    let mut mentions = Vec::new();
    for chunk in chunks {
        let (normalized_body, source_spans) = normalize_with_spans(chunk.body);
        matcher.find_in_chunk(chunk, &normalized_body, &source_spans, &mut mentions)?;
    }
    mentions.sort_unstable_by(|left, right| {
        (
            left.chunk_id,
            left.start_offset,
            left.end_offset,
            &left.stock_id,
        )
            .cmp(&(
                right.chunk_id,
                right.start_offset,
                right.end_offset,
                &right.stock_id,
            ))
    });
    Ok(mentions)
}

impl StockMatcher {
    fn new(stocks: &[RegisteredStock]) -> Self {
        let mut matcher = Self {
            patterns: Vec::new(),
            nodes: vec![TrieNode::default()],
        };
        for stock in stocks {
            let characters = normalize(&stock.name).chars().collect::<Vec<_>>();
            if characters.is_empty() {
                continue;
            }
            let pattern_index = matcher.patterns.len();
            let mut node_index = 0;
            for character in &characters {
                let Some(next_index) = matcher.nodes[node_index].transitions.get(character) else {
                    let next_index = matcher.nodes.len();
                    matcher.nodes.push(TrieNode::default());
                    matcher.nodes[node_index]
                        .transitions
                        .insert(*character, next_index);
                    node_index = next_index;
                    continue;
                };
                node_index = *next_index;
            }
            matcher.nodes[node_index].outputs.push(pattern_index);
            matcher.patterns.push(StockPattern {
                stock_id: stock.id.clone(),
                characters,
            });
        }
        matcher.build_failure_links();
        matcher
    }

    fn build_failure_links(&mut self) {
        let mut queue = self.nodes[0]
            .transitions
            .values()
            .copied()
            .collect::<VecDeque<_>>();
        while let Some(node_index) = queue.pop_front() {
            let transitions = self.nodes[node_index]
                .transitions
                .iter()
                .map(|(character, child_index)| (*character, *child_index))
                .collect::<Vec<_>>();
            for (character, child_index) in transitions {
                let mut failure = self.nodes[node_index].failure;
                while failure != 0 && !self.nodes[failure].transitions.contains_key(&character) {
                    failure = self.nodes[failure].failure;
                }
                let failure = self.nodes[failure]
                    .transitions
                    .get(&character)
                    .copied()
                    .unwrap_or(0);
                self.nodes[child_index].failure = failure;
                let suffix_outputs = self.nodes[failure].outputs.clone();
                self.nodes[child_index].outputs.extend(suffix_outputs);
                queue.push_back(child_index);
            }
        }
    }

    fn find_in_chunk(
        &self,
        chunk: &ChunkText<'_>,
        normalized_body: &[char],
        source_spans: &[SourceSpan],
        mentions: &mut Vec<DocumentMention>,
    ) -> Result<(), InvalidOffset> {
        let mut node_index = 0;
        for (end_index, character) in normalized_body.iter().enumerate() {
            while node_index != 0 && !self.nodes[node_index].transitions.contains_key(character) {
                node_index = self.nodes[node_index].failure;
            }
            node_index = self.nodes[node_index]
                .transitions
                .get(character)
                .copied()
                .unwrap_or(0);
            for pattern_index in &self.nodes[node_index].outputs {
                let pattern = &self.patterns[*pattern_index];
                let start_index = end_index + 1 - pattern.characters.len();
                let matched_spans = &source_spans[start_index..=end_index];
                let start_offset = matched_spans
                    .iter()
                    .map(|span| span.start)
                    .min()
                    .ok_or(InvalidOffset)?;
                let end_offset = matched_spans
                    .iter()
                    .map(|span| span.end)
                    .max()
                    .ok_or(InvalidOffset)?;
                mentions.push(DocumentMention {
                    chunk_id: chunk.id,
                    stock_id: pattern.stock_id.clone(),
                    start_offset: add_offset(chunk.start_offset, start_offset)?,
                    end_offset: add_offset(chunk.start_offset, end_offset)?,
                });
            }
        }
        Ok(())
    }
}

fn normalize(value: &str) -> String {
    value
        .nfkd()
        .flat_map(char::to_lowercase)
        .collect::<String>()
}

fn normalize_with_spans(value: &str) -> (Vec<char>, Vec<SourceSpan>) {
    let mut decomposed = Vec::new();
    for (start, character) in value.char_indices() {
        let span = SourceSpan {
            start,
            end: start + character.len_utf8(),
        };
        decomposed.extend(
            character
                .to_string()
                .nfkd()
                .map(|character| (character, span)),
        );
    }

    let mut mark_start = 0;
    for index in 0..=decomposed.len() {
        if index == decomposed.len() || canonical_combining_class(decomposed[index].0) == 0 {
            decomposed[mark_start..index]
                .sort_by_key(|(character, _)| canonical_combining_class(*character));
            mark_start = index + 1;
        }
    }

    let mut characters = Vec::new();
    let mut source_spans = Vec::new();
    for (character, span) in decomposed {
        for lower_character in character.to_lowercase() {
            characters.push(lower_character);
            source_spans.push(span);
        }
    }
    (characters, source_spans)
}

fn add_offset(base: i32, local_offset: usize) -> Result<i32, InvalidOffset> {
    let local_offset = i32::try_from(local_offset).map_err(|_| InvalidOffset)?;
    base.checked_add(local_offset).ok_or(InvalidOffset)
}
