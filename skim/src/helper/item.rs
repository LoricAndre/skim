use crate::ansi::ANSIParser;
use crate::field::{parse_matching_fields, parse_transform_fields, FieldRange};
use crate::{AnsiString, DisplayContext, Matches, SkimItem};
use regex::Regex;
use tuikit::prelude::Attr;

//------------------------------------------------------------------------------
/// An item will store everything that one line input will need to be operated and displayed.
///
/// What's special about an item?
/// The simplest version of an item is a line of string, but things are getting more complex:
/// - The conversion of lower/upper case is slow in rust, because it involds unicode.
/// - We may need to interpret the ANSI codes in the text.
/// - The text can be transformed and limited while searching.
///
/// About the ANSI, we made assumption that it is linewise, that means no ANSI codes will affect
/// more than one line.
#[derive(Debug)]
pub struct DefaultSkimItem {
    /// The text that will be output when user press `enter`
    /// `Some(..)` => the original input is transformed, could not output `text` directly
    /// `None` => that it is safe to output `text` directly
    orig_text: Option<String>,

    /// The text that will be shown on screen and matched.
    text: AnsiString,

    // Option<Box<_>> to reduce memory use in normal cases where no matching ranges are specified.
    #[allow(clippy::box_collection)]
    matching_ranges: Option<Box<Vec<(usize, usize)>>>,
}

impl DefaultSkimItem {
    pub fn new(
        orig_text: String,
        ansi_enabled: bool,
        trans_fields: &[FieldRange],
        matching_fields: &[FieldRange],
        delimiter: &Regex,
    ) -> Self {
        let using_transform_fields = !trans_fields.is_empty();
        debug!("transforming: {}, ansi: {}", using_transform_fields, ansi_enabled);

        //        transformed | ANSI             | output
        //------------------------------------------------------
        //                    +- T -> trans+ANSI | ANSI
        //                    |                  |
        //      +- T -> trans +- F -> trans      | orig
        // orig |                                |
        //      +- F -> orig  +- T -> ANSI     ==| ANSI
        //                    |                  |
        //                    +- F -> orig       | orig

        let mut ansi_parser: ANSIParser = Default::default();

        let (orig_text, text) = if using_transform_fields && ansi_enabled {
            // ansi and transform
            let transformed = ansi_parser.parse_ansi(&parse_transform_fields(delimiter, &orig_text, trans_fields));
            debug!("transformed ansi {:#?}", transformed);
            (Some(orig_text), transformed)
        } else if using_transform_fields {
            // transformed, not ansi
            let transformed = parse_transform_fields(delimiter, &orig_text, trans_fields).into();
            debug!("transformed raw {:#?}", transformed);
            (Some(orig_text), transformed)
        } else if ansi_enabled {
            // not transformed, ansi
            (None, ansi_parser.parse_ansi(&orig_text))
        } else {
            // normal case
            (None, orig_text.into())
        };

        let matching_ranges = if !matching_fields.is_empty() {
            Some(Box::new(parse_matching_fields(
                delimiter,
                text.stripped(),
                matching_fields,
            )))
        } else {
            None
        };

        DefaultSkimItem {
            orig_text,
            text,
            matching_ranges,
        }
    }
}

impl SkimItem for DefaultSkimItem {
    #[inline]
    fn text(&self) -> &str {
        self.text.stripped()
    }

    fn output(&self) -> String {
        if let Some(orig_text) = self.orig_text.clone() {
            if self.text.has_attrs() {
                let mut ansi_parser: ANSIParser = Default::default();
                let text = ansi_parser.parse_ansi(&orig_text);
                text.into_inner()
            } else {
                self.orig_text.clone().unwrap()
            }
        } else {
            self.text.clone().into_inner()
        }
    }

    fn get_matching_ranges(&self) -> Option<&[(usize, usize)]> {
        self.matching_ranges.as_ref().map(|vec| vec as &[(usize, usize)])
    }

    fn display<'a>(&'a self, context: DisplayContext<'a>) -> AnsiString {
        let new_fragments: Vec<(Attr, (usize, usize))> = match context.matches {
            Matches::CharIndices(indices) => indices
                .iter()
                .map(|&idx| (context.highlight_attr, (idx, idx + 1)))
                .collect(),
            Matches::CharRange(start, end) => vec![(context.highlight_attr, (start, end))],
            Matches::ByteRange(start, end) => {
                let ch_start = context.text[..start].chars().count();
                let ch_end = ch_start + context.text[start..end].chars().count();
                vec![(context.highlight_attr, (ch_start, ch_end))]
            }
            Matches::None => vec![],
        };
        let mut ret = self.text.clone();
        ret.override_attrs(new_fragments);
        ret
    }
}
