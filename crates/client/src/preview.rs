use pulldown_cmark::{Options, Parser, html};
use web_sys::Element;

pub fn render(element: &Element, markdown: &str) {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_TABLES);

    let parser = Parser::new_ext(markdown, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);

    element.set_inner_html(&ammonia::clean(&output));
}
