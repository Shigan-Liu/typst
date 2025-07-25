use std::num::NonZeroUsize;
use std::str::FromStr;

use typst_utils::NonZeroExt;

use crate::diag::{StrResult, bail};
use crate::engine::Engine;
use crate::foundations::{
    Content, Label, NativeElement, Packed, ShowSet, Smart, StyleChain, Styles, cast,
    elem, scope,
};
use crate::introspection::{Count, CounterUpdate, Locatable, Location};
use crate::layout::{Abs, Em, Length, Ratio};
use crate::model::{Numbering, NumberingPattern, ParElem};
use crate::text::{TextElem, TextSize};
use crate::visualize::{LineElem, Stroke};

/// A footnote.
///
/// Includes additional remarks and references on the same page with footnotes.
/// A footnote will insert a superscript number that links to the note at the
/// bottom of the page. Notes are numbered sequentially throughout your document
/// and can break across multiple pages.
///
/// To customize the appearance of the entry in the footnote listing, see
/// [`footnote.entry`]($footnote.entry). The footnote itself is realized as a
/// normal superscript, so you can use a set rule on the [`super`] function to
/// customize it. You can also apply a show rule to customize only the footnote
/// marker (superscript number) in the running text.
///
/// # Example
/// ```example
/// Check the docs for more details.
/// #footnote[https://typst.app/docs]
/// ```
///
/// The footnote automatically attaches itself to the preceding word, even if
/// there is a space before it in the markup. To force space, you can use the
/// string `[#" "]` or explicit [horizontal spacing]($h).
///
/// By giving a label to a footnote, you can have multiple references to it.
///
/// ```example
/// You can edit Typst documents online.
/// #footnote[https://typst.app/app] <fn>
/// Checkout Typst's website. @fn
/// And the online app. #footnote(<fn>)
/// ```
///
/// _Note:_ Set and show rules in the scope where `footnote` is called may not
/// apply to the footnote's content. See [here][issue] for more information.
///
/// [issue]: https://github.com/typst/typst/issues/1467#issuecomment-1588799440
#[elem(scope, Locatable, Count)]
pub struct FootnoteElem {
    /// How to number footnotes.
    ///
    /// By default, the footnote numbering continues throughout your document.
    /// If you prefer per-page footnote numbering, you can reset the footnote
    /// [counter] in the page [header]($page.header). In the future, there might
    /// be a simpler way to achieve this.
    ///
    /// ```example
    /// #set footnote(numbering: "*")
    ///
    /// Footnotes:
    /// #footnote[Star],
    /// #footnote[Dagger]
    /// ```
    #[default(Numbering::Pattern(NumberingPattern::from_str("1").unwrap()))]
    pub numbering: Numbering,

    /// The content to put into the footnote. Can also be the label of another
    /// footnote this one should point to.
    #[required]
    pub body: FootnoteBody,
}

#[scope]
impl FootnoteElem {
    #[elem]
    type FootnoteEntry;
}

impl FootnoteElem {
    /// Creates a new footnote that the passed content as its body.
    pub fn with_content(content: Content) -> Self {
        Self::new(FootnoteBody::Content(content))
    }

    /// Creates a new footnote referencing the footnote with the specified label.
    pub fn with_label(label: Label) -> Self {
        Self::new(FootnoteBody::Reference(label))
    }

    /// Creates a new footnote referencing the footnote with the specified label,
    /// with the other fields from the current footnote cloned.
    pub fn into_ref(&self, label: Label) -> Self {
        Self {
            body: FootnoteBody::Reference(label),
            ..self.clone()
        }
    }

    /// Tests if this footnote is a reference to another footnote.
    pub fn is_ref(&self) -> bool {
        matches!(self.body, FootnoteBody::Reference(_))
    }

    /// Returns the content of the body of this footnote if it is not a ref.
    pub fn body_content(&self) -> Option<&Content> {
        match &self.body {
            FootnoteBody::Content(content) => Some(content),
            _ => None,
        }
    }
}

impl Packed<FootnoteElem> {
    /// Returns the location of the definition of this footnote.
    pub fn declaration_location(&self, engine: &Engine) -> StrResult<Location> {
        match self.body {
            FootnoteBody::Reference(label) => {
                let element = engine.introspector.query_label(label)?;
                let footnote = element
                    .to_packed::<FootnoteElem>()
                    .ok_or("referenced element should be a footnote")?;
                if self.location() == footnote.location() {
                    bail!("footnote cannot reference itself");
                }
                footnote.declaration_location(engine)
            }
            _ => Ok(self.location().unwrap()),
        }
    }
}

impl Count for Packed<FootnoteElem> {
    fn update(&self) -> Option<CounterUpdate> {
        (!self.is_ref()).then(|| CounterUpdate::Step(NonZeroUsize::ONE))
    }
}

/// The body of a footnote can be either some content or a label referencing
/// another footnote.
#[derive(Debug, Clone, PartialEq, Hash)]
pub enum FootnoteBody {
    Content(Content),
    Reference(Label),
}

cast! {
    FootnoteBody,
    self => match self {
        Self::Content(v) => v.into_value(),
        Self::Reference(v) => v.into_value(),
    },
    v: Content => Self::Content(v),
    v: Label => Self::Reference(v),
}

/// An entry in a footnote list.
///
/// This function is not intended to be called directly. Instead, it is used in
/// set and show rules to customize footnote listings.
///
/// ```example
/// #show footnote.entry: set text(red)
///
/// My footnote listing
/// #footnote[It's down here]
/// has red text!
/// ```
///
/// _Note:_ Footnote entry properties must be uniform across each page run (a
/// page run is a sequence of pages without an explicit pagebreak in between).
/// For this reason, set and show rules for footnote entries should be defined
/// before any page content, typically at the very start of the document.
#[elem(name = "entry", title = "Footnote Entry", ShowSet)]
pub struct FootnoteEntry {
    /// The footnote for this entry. Its location can be used to determine
    /// the footnote counter state.
    ///
    /// ```example
    /// #show footnote.entry: it => {
    ///   let loc = it.note.location()
    ///   numbering(
    ///     "1: ",
    ///     ..counter(footnote).at(loc),
    ///   )
    ///   it.note.body
    /// }
    ///
    /// Customized #footnote[Hello]
    /// listing #footnote[World! 🌏]
    /// ```
    #[required]
    pub note: Packed<FootnoteElem>,

    /// The separator between the document body and the footnote listing.
    ///
    /// ```example
    /// #set footnote.entry(
    ///   separator: repeat[.]
    /// )
    ///
    /// Testing a different separator.
    /// #footnote[
    ///   Unconventional, but maybe
    ///   not that bad?
    /// ]
    /// ```
    #[default(
        LineElem::new()
            .with_length(Ratio::new(0.3).into())
            .with_stroke(Stroke {
                thickness: Smart::Custom(Abs::pt(0.5).into()),
                ..Default::default()
            })
            .pack()
    )]
    pub separator: Content,

    /// The amount of clearance between the document body and the separator.
    ///
    /// ```example
    /// #set footnote.entry(clearance: 3em)
    ///
    /// Footnotes also need ...
    /// #footnote[
    ///   ... some space to breathe.
    /// ]
    /// ```
    #[default(Em::new(1.0).into())]
    pub clearance: Length,

    /// The gap between footnote entries.
    ///
    /// ```example
    /// #set footnote.entry(gap: 0.8em)
    ///
    /// Footnotes:
    /// #footnote[Spaced],
    /// #footnote[Apart]
    /// ```
    #[default(Em::new(0.5).into())]
    pub gap: Length,

    /// The indent of each footnote entry.
    ///
    /// ```example
    /// #set footnote.entry(indent: 0em)
    ///
    /// Footnotes:
    /// #footnote[No],
    /// #footnote[Indent]
    /// ```
    #[default(Em::new(1.0).into())]
    pub indent: Length,
}

impl ShowSet for Packed<FootnoteEntry> {
    fn show_set(&self, _: StyleChain) -> Styles {
        let mut out = Styles::new();
        out.set(ParElem::leading, Em::new(0.5).into());
        out.set(TextElem::size, TextSize(Em::new(0.85).into()));
        out
    }
}

cast! {
    FootnoteElem,
    v: Content => v.unpack::<Self>().unwrap_or_else(Self::with_content)
}
