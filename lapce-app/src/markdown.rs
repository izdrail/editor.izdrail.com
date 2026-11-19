use floem::text::{
    Attrs, AttrsList, FamilyOwned, LineHeightValue, Style, TextLayout, Weight,
};
use floem::{peniko::kurbo::Point, View};
use lapce_core::{language::LapceLanguage, syntax::Syntax};
use lapce_xi_rope::Rope;
use lsp_types::MarkedString;
use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag};
use smallvec::SmallVec;

use crate::config::{LapceConfig, color::LapceColor};

#[derive(Clone)]
pub enum MarkdownContent {
    Text { layout: TextLayout, links: Vec<MdLink> },
    Image { url: String, title: String },
    Separator,
}

/// A markdown link's byte range inside its text block (item 443).
#[derive(Clone)]
pub struct MdLink {
    pub start: usize,
    pub end: usize,
    pub url: String,
}

pub fn parse_markdown(
    text: &str,
    line_height: f64,
    config: &LapceConfig,
) -> Vec<MarkdownContent> {
    let mut res = Vec::new();

    let mut current_text = String::new();
    let code_font_family: Vec<FamilyOwned> =
        FamilyOwned::parse_list(&config.editor.font_family).collect();

    let default_attrs = Attrs::new()
        .color(config.color(LapceColor::EDITOR_FOREGROUND))
        .font_size(config.ui.font_size() as f32)
        .line_height(LineHeightValue::Normal(line_height as f32));
    let mut attr_list = AttrsList::new(default_attrs.clone());

    let mut builder_dirty = false;

    let mut pos = 0;

    // Links of the block currently built; flushed with it.
    let mut block_links: Vec<MdLink> = Vec::new();

    let mut tag_stack: SmallVec<[(usize, Tag); 4]> = SmallVec::new();

    let parser = Parser::new_ext(
        text,
        Options::ENABLE_TABLES
            | Options::ENABLE_FOOTNOTES
            | Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_HEADING_ATTRIBUTES,
    );
    let mut last_text = CowStr::from("");
    // Whether we should add a newline on the next entry
    // This is used so that we don't emit newlines at the very end of the generation
    let mut add_newline = false;
    for event in parser {
        // Add the newline since we're going to be outputting more
        if add_newline {
            current_text.push('\n');
            builder_dirty = true;
            pos += 1;
            add_newline = false;
        }

        match event {
            Event::Start(tag) => {
                tag_stack.push((pos, tag));
            }
            Event::End(end_tag) => {
                if let Some((start_offset, tag)) = tag_stack.pop() {
                    if end_tag != tag.to_end() {
                        tracing::warn!("Mismatched markdown tag");
                        continue;
                    }

                    if let Some(attrs) = attribute_for_tag(
                        default_attrs.clone(),
                        &tag,
                        &code_font_family,
                        config,
                    ) {
                        attr_list
                            .add_span(start_offset..pos.max(start_offset), attrs);
                    }

                    if let Tag::Link { dest_url, .. } = &tag {
                        block_links.push(MdLink {
                            start: start_offset,
                            end: pos.max(start_offset),
                            url: dest_url.to_string(),
                        });
                    }

                    if should_add_newline_after_tag(&tag) {
                        add_newline = true;
                    }

                    match &tag {
                        Tag::CodeBlock(kind) => {
                            let language =
                                if let CodeBlockKind::Fenced(language) = kind {
                                    md_language_to_lapce_language(language)
                                } else {
                                    None
                                };

                            highlight_as_code(
                                &mut attr_list,
                                default_attrs.clone().family(&code_font_family),
                                language,
                                &last_text,
                                start_offset,
                                config,
                            );
                            builder_dirty = true;
                        }
                        Tag::Image {
                            link_type: _,
                            dest_url: dest,
                            title,
                            id: _,
                        } => {
                            // TODO: Are there any link types that would change how the
                            // image is rendered?

                            if builder_dirty {
                                let mut text_layout = TextLayout::new();
                                text_layout.set_text(&current_text, attr_list, None);
                                res.push(MarkdownContent::Text {
                                layout: text_layout,
                                links: std::mem::take(&mut block_links),
                            });
                                attr_list = AttrsList::new(default_attrs.clone());
                                current_text.clear();
                                pos = 0;
                                builder_dirty = false;
                            }

                            res.push(MarkdownContent::Image {
                                url: dest.to_string(),
                                title: title.to_string(),
                            });
                        }
                        _ => {
                            // Presumably?
                            builder_dirty = true;
                        }
                    }
                } else {
                    tracing::warn!("Unbalanced markdown tag")
                }
            }
            Event::Text(text) => {
                if let Some((_, tag)) = tag_stack.last() {
                    if should_skip_text_in_tag(tag) {
                        continue;
                    }
                }
                current_text.push_str(&text);
                pos += text.len();
                last_text = text;
                builder_dirty = true;
            }
            Event::Code(text) => {
                attr_list.add_span(
                    pos..pos + text.len(),
                    default_attrs.clone().family(&code_font_family),
                );
                current_text.push_str(&text);
                pos += text.len();
                builder_dirty = true;
            }
            // TODO: Some minimal 'parsing' of html could be useful here, since some things use
            // basic html like `<code>text</code>`.
            Event::Html(text) => {
                attr_list.add_span(
                    pos..pos + text.len(),
                    default_attrs
                        .clone()
                        .family(&code_font_family)
                        .color(config.color(LapceColor::MARKDOWN_BLOCKQUOTE)),
                );
                current_text.push_str(&text);
                pos += text.len();
                builder_dirty = true;
            }
            Event::HardBreak => {
                current_text.push('\n');
                pos += 1;
                builder_dirty = true;
            }
            Event::SoftBreak => {
                current_text.push(' ');
                pos += 1;
                builder_dirty = true;
            }
            Event::Rule => {}
            Event::FootnoteReference(_text) => {}
            Event::TaskListMarker(_text) => {}
            Event::InlineHtml(_) => {} // TODO(panekj): Implement
            Event::InlineMath(_) => {} // TODO(panekj): Implement
            Event::DisplayMath(_) => {} // TODO(panekj): Implement
        }
    }

    if builder_dirty {
        let mut text_layout = TextLayout::new();
        text_layout.set_text(&current_text, attr_list, None);
        res.push(MarkdownContent::Text {
                                layout: text_layout,
                                links: std::mem::take(&mut block_links),
                            });
    }

    res
}

fn attribute_for_tag<'a>(
    default_attrs: Attrs<'a>,
    tag: &Tag,
    code_font_family: &'a [FamilyOwned],
    config: &LapceConfig,
) -> Option<Attrs<'a>> {
    use pulldown_cmark::HeadingLevel;
    match tag {
        Tag::Heading {
            level,
            id: _,
            classes: _,
            attrs: _,
        } => {
            // The size calculations are based on the em values given at
            // https://drafts.csswg.org/css2/#html-stylesheet
            let font_scale = match level {
                HeadingLevel::H1 => 2.0,
                HeadingLevel::H2 => 1.5,
                HeadingLevel::H3 => 1.17,
                HeadingLevel::H4 => 1.0,
                HeadingLevel::H5 => 0.83,
                HeadingLevel::H6 => 0.75,
            };
            let font_size = font_scale * config.ui.font_size() as f64;
            Some(
                default_attrs
                    .font_size(font_size as f32)
                    .weight(Weight::BOLD),
            )
        }
        Tag::BlockQuote(_block_quote) => Some(
            default_attrs
                .style(Style::Italic)
                .color(config.color(LapceColor::MARKDOWN_BLOCKQUOTE)),
        ),
        Tag::CodeBlock(_) => Some(default_attrs.family(code_font_family)),
        Tag::Emphasis => Some(default_attrs.style(Style::Italic)),
        Tag::Strong => Some(default_attrs.weight(Weight::BOLD)),
        // TODO: Strikethrough support
        Tag::Link {
            link_type: _,
            dest_url: _,
            title: _,
            id: _,
        } => {
            // TODO: Link support
            Some(default_attrs.color(config.color(LapceColor::EDITOR_LINK)))
        }
        // All other tags are currently ignored
        _ => None,
    }
}

/// Decides whether newlines should be added after a specific markdown tag
fn should_add_newline_after_tag(tag: &Tag) -> bool {
    !matches!(
        tag,
        Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. }
    )
}

/// Whether it should skip the text node after a specific tag  
/// For example, images are skipped because it emits their title as a separate text node.  
fn should_skip_text_in_tag(tag: &Tag) -> bool {
    matches!(tag, Tag::Image { .. })
}

fn md_language_to_lapce_language(lang: &str) -> Option<LapceLanguage> {
    // TODO: There are many other names commonly used that should be supported
    LapceLanguage::from_name(lang)
}

/// Highlight the text in a richtext builder like it was a markdown codeblock
pub fn highlight_as_code(
    attr_list: &mut AttrsList,
    default_attrs: Attrs,
    language: Option<LapceLanguage>,
    text: &str,
    start_offset: usize,
    config: &LapceConfig,
) {
    let syntax = language.map(Syntax::from_language);

    let styles = syntax
        .map(|mut syntax| {
            syntax.parse(0, Rope::from(text), None);
            syntax.styles
        })
        .unwrap_or(None);

    if let Some(styles) = styles {
        for (range, style) in styles.iter() {
            if let Some(color) = style
                .fg_color
                .as_ref()
                .and_then(|fg| config.style_color(fg))
            {
                attr_list.add_span(
                    start_offset + range.start..start_offset + range.end,
                    default_attrs.clone().color(color),
                );
            }
        }
    }
}

pub fn from_marked_string(
    text: MarkedString,
    config: &LapceConfig,
) -> Vec<MarkdownContent> {
    match text {
        MarkedString::String(text) => parse_markdown(&text, 1.8, config),
        // This is a short version of a code block
        MarkedString::LanguageString(code) => {
            // TODO: We could simply construct the MarkdownText directly
            // Simply construct the string as if it was written directly
            parse_markdown(
                &format!("```{}\n{}\n```", code.language, code.value),
                1.8,
                config,
            )
        }
    }
}

pub fn from_plaintext(
    text: &str,
    line_height: f64,
    config: &LapceConfig,
) -> Vec<MarkdownContent> {
    let mut text_layout = TextLayout::new();
    text_layout.set_text(
        text,
        AttrsList::new(
            Attrs::new()
                .color(config.color(LapceColor::EDITOR_FOREGROUND))
                .font_size(config.ui.font_size() as f32)
                .line_height(LineHeightValue::Normal(line_height as f32)),
        ),
        None,
    );
    vec![MarkdownContent::Text {
        layout: text_layout,
        links: Vec::new(),
    }]
}

/// Rich text whose markdown links open on click (item 443): a
/// pointer-down whose byte offset lands inside a link's range opens
/// its URL with the system handler. The hit layout is a private copy
/// resized to the view's last width, so wrapping matches what is on
/// screen.
pub fn linked_text(layout: TextLayout, links: Vec<MdLink>) -> impl View {
    use floem::{
        event::{Event, EventListener},
        peniko::kurbo::Point,
        views::{rich_text, Decorators},
    };
    let shown = layout.clone();
    let hit_layout = std::cell::RefCell::new(layout);
    let view = rich_text(move || shown.clone());
    let id = view.id();
    view.on_event_cont(EventListener::PointerDown, move |event| {
        if links.is_empty() {
            return;
        }
        if let Event::PointerDown(pe) = event {
            // floem offsets pointer events into the view's local coordinate
            // space during dispatch, so pe.pos is already local to this view.
            let size = id.get_size().unwrap_or_default();
            let local = Point::new(pe.pos.x, pe.pos.y);
            let mut hit = hit_layout.borrow_mut();
            hit.set_size(size.width as f32, f32::MAX);
            let point = hit.hit_point(local);
            if !point.is_inside {
                return;
            }
            // cosmic hit indices are relative to their source line; the link
            // ranges are byte offsets in the whole block text.
            let index = hit
                .lines_range()
                .get(point.line)
                .map(|r| r.start + point.index)
                .unwrap_or(point.index);
            if let Some(link) = links
                .iter()
                .find(|l| index >= l.start && index < l.end)
            {
                if let Err(e) = open::that_detached(&link.url) {
                    tracing::error!("failed to open {}: {e}", link.url);
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_ranges_cover_their_text() {
        let config = LapceConfig::default();
        let out = parse_markdown(
            "See [the docs](https://example.com/ref) now",
            1.8,
            &config,
        );
        let [MarkdownContent::Text { layout, links }] = &out[..] else {
            panic!("one text block");
        };
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/ref");
        // "See " is 4 bytes, "the docs" the next 8.
        assert_eq!((links[0].start, links[0].end), (4, 12));
        // The click path round-trips: a point inside the range hits
        // an offset inside the range.
        let mut hit = layout.clone();
        hit.set_size(600.0, f32::MAX);
        let point = hit.hit_position(8).point;
        assert!((4..12).contains(&hit.hit_point(point).index));
        // A point past the block's text is not a link hit.
        assert!(!hit.hit_point(Point::new(590.0, 1.0)).is_inside
            || hit.hit_point(Point::new(590.0, 1.0)).index >= 12);
    }

    #[test]
    fn plain_text_has_no_links() {
        let config = LapceConfig::default();
        let out = parse_markdown("no links here", 1.8, &config);
        let [MarkdownContent::Text { links, .. }] = &out[..] else {
            panic!("one text block");
        };
        assert!(links.is_empty());
        let out = from_plaintext("no links here either", 1.8, &config);
        let [MarkdownContent::Text { links, .. }] = &out[..] else {
            panic!("one text block");
        };
        assert!(links.is_empty());
    }
}
