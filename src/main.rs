fn main() {
    let file = include_str!("../example.xml");

    let tokens = parse_file(file).expect("Failed to lex");

    for token in &tokens {
        println!("{:?}", token);
    }

    let tree = parse_node_tree(tokens).expect("Failed to parse tree");

    println!("{:#?}", tree);
}

#[derive(Debug)]
struct Document {
    prolog: Option<Vec<Attribute>>,
    children: Vec<Node>,
}

#[derive(Debug)]
enum Node {
    Text(String),
    Element(Element),
}

#[derive(Debug)]
struct Element {
    tag_name: String,
    attributes: Vec<Attribute>,
    children: Vec<Node>,
}

//TODO: use iterator
fn parse_node_tree(tokens: Vec<Token>) -> Result<Document, String> {
    let prolog: Option<Vec<Attribute>> = match tokens.first() {
        Some(Token::Tag(tag)) if tag.name == "?xml" => Some(tag.attributes.clone()),
        _ => None,
    };

    let mut tokens = tokens.into_iter();
    if prolog.is_some() {
        tokens.next();
    }

    let children = parse_node_tree_part(&mut tokens, 0, None)?;
    let prolog = None;
    Ok(Document { prolog, children })
}

fn parse_node_tree_part<I>(
    tokens: &mut I,
    depth: usize,
    current_tag_name: Option<&str>,
) -> Result<Vec<Node>, String>
where
    I: Iterator<Item = Token>,
{
    let mut nodes = Vec::new();

    while let Some(token) = tokens.next() {
        match token {
            Token::Text(text) => nodes.push(Node::Text(text)),

            Token::Tag(tag) => {
                let TagToken {
                    is_closing,
                    name,
                    attributes,
                } = tag;

                if name == "?xml" {
                    return Err(format!(
                        "Unexpected XML prolog. Prolog must occur at beginning of file"
                    ));
                }

                if is_closing {
                    if let Some(current) = current_tag_name {
                        if current != name {
                            return Err(format!(
                                "Mismatched closing tag `</{}>. Does not match `<{}>`",
                                name, current,
                            ));
                        }
                    }

                    if depth == 0 {
                        return Err(format!(
                            "Unexpected closing tag `</{}>. Expected end of file`",
                            name
                        ));
                    }
                    return Ok(nodes);
                }

                if depth == 0 && !nodes.is_empty() {
                    return Err(format!(
                        "Unexpected opening tag. Expected end of file. Only one root node is allowed"
                    ));
                }

                let children = parse_node_tree_part(tokens, depth + 1, Some(&name))?;

                let node = Node::Element(Element {
                    tag_name: name,
                    attributes,
                    children,
                });
                nodes.push(node);
            }
        }
    }

    if depth > 0 {
        // If depth > 0, then current_tag_name must be Some
        let current = current_tag_name.unwrap(); // Bruh
        return Err(format!(
            "Unexpected end of file. Expected closing tag </{}>.",
            current
        ));
    }

    Ok(nodes)
}

#[derive(Debug)]
enum Token {
    Text(String),
    Tag(TagToken),
}

struct TagToken {
    is_closing: bool,
    name: String,
    attributes: Vec<Attribute>,
}

type Attribute = (String, Option<String>);

impl std::fmt::Debug for TagToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "<{} {:?} {:?}>",
            if self.is_closing { "/" } else { "" },
            self.name,
            self.attributes
        )
    }
}

fn parse_file(file: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();

    let mut current_token = String::new();
    let mut is_tag = false;
    let mut is_comment = false;

    let mut chars = file.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '<' if !is_comment => {
                if is_tag {
                    return Err(format!("Unexpected `<`"));
                }
                is_tag = true;
                if !current_token.trim().is_empty() {
                    tokens.push(Token::Text(parse_text(&current_token)));
                }
                current_token = String::new();
            }
            '>' if !is_comment => {
                if !is_tag {
                    return Err(format!("Unexpected `>`"));
                }
                is_tag = false;
                if !current_token.is_empty() {
                    tokens.push(Token::Tag(parse_tag_token(&current_token)?));
                    current_token = String::new();
                }
            }
            _ => {
                if chars.as_str().starts_with("<!--") {
                    is_comment = true;
                } else if chars.as_str().starts_with("-->") {
                    is_comment = false;
                    chars.nth("-->".len() - 1);
                } else if !is_comment {
                    current_token.push(ch);
                }
            }
        }
    }

    if !current_token.trim().is_empty() {
        if is_tag {
            return Err(format!("Unexpected end of file. Expected `>`"));
        }
        tokens.push(Token::Text(parse_text(&current_token)));
    }

    Ok(tokens)
}

fn parse_text(input: &str) -> String {
    let mut output = String::new();
    let mut current_entity: Option<String> = None;

    for ch in input.chars() {
        if let Some(ref mut entity) = current_entity {
            if ch == ';' || ch.is_whitespace() {
                if let Some(entity_value) = parse_text_entity(entity) {
                    output += entity_value;
                } else {
                    eprintln!("[warning] unknown text entity `&{};`", entity);
                    output.push('&');
                    output += entity;
                    output.push(';');
                }
                if ch.is_whitespace() {
                    output.push(ch);
                }
                current_entity = None;
            } else {
                entity.push(ch);
            }
        } else {
            if ch == '&' {
                current_entity = Some(String::new());
            } else {
                output.push(ch);
            }
        }
    }

    output
}

fn parse_text_entity(input: &str) -> Option<&'static str> {
    // TODO: Hex codes etc.

    Some(match input {
        "lt" => "<",
        "gt" => ">",
        "amp" => "&",
        "apos" => "'",
        "quot" => "\"",

        _ => return None,
    })
}

fn parse_tag_token(mut token: &str) -> Result<TagToken, String> {
    if token.chars().next().is_some_and(|ch| ch.is_whitespace()) {
        return Err(format!("Unexpected whitespace in tag `<{}>`", token));
    }

    let is_closing = token.starts_with('/');
    if is_closing {
        let mut chars = token.chars();
        chars.next();
        token = chars.as_str();

        if token.chars().next().is_some_and(|ch| ch.is_whitespace()) {
            return Err(format!(
                "Unexpected whitespace in tag `</{}>`, after backslash",
                token
            ));
        }
    }

    let (name, attributes) = match token.find(' ') {
        Some(index) => {
            let (name, attr_str) = token.split_at(index);
            (name, parse_tag_attributes(attr_str)?)
        }
        None => (token, Vec::new()),
    };

    Ok(TagToken {
        is_closing,
        name: name.to_string(),
        attributes,
    })
}

fn parse_tag_attributes(string: &str) -> Result<Vec<Attribute>, String> {
    let mut attrs = Vec::<Attribute>::new();

    let mut key_opt: Option<(bool, String)> = None;
    let mut value_opt: Option<(char, String)> = None;

    let mut chars = string.chars();
    while let Some(ch) = chars.next() {
        match key_opt {
            None => {
                if ch.is_whitespace() {
                    continue;
                }
                if ch == '=' {
                    return Err(format!("Unexpected `=`. Expected start of attribute key"));
                }
                key_opt = Some((false, ch.to_string()));
            }

            Some((ref mut was_whitespace, ref mut key)) => match value_opt {
                None => {
                    if ch.is_whitespace() {
                        *was_whitespace = true;
                        continue;
                    }
                    if *was_whitespace && ch != '=' {
                        let key = key_opt.unwrap().1; // Bruh
                        attrs.push((key, None));
                        key_opt = Some((false, ch.to_string()));
                        continue;
                    }

                    if ch != '=' {
                        key.push(ch);
                        continue;
                    }

                    let quote = loop {
                        let ch = chars.next();
                        if ch.is_some_and(|ch| ch.is_whitespace()) {
                            continue;
                        }
                        break ch;
                    };

                    let Some(quote) = quote else {
                        return Err(format!("Unexpected end of tag. Expected `'` or `\"`"));
                    };
                    if quote != '"' && quote != '\'' {
                        return Err(format!("Unexpected `{}`. Expected `'` or `\"`", quote));
                    }

                    value_opt = Some((quote, String::new()));
                }

                Some((quote, ref mut value)) => {
                    if ch != quote {
                        value.push(ch);
                        continue;
                    }

                    let key = key_opt.unwrap().1; // Bruh
                    let value = parse_tag_attribute_value(value)?;
                    attrs.push((key, Some(value)));

                    key_opt = None;
                    value_opt = None;
                }
            },
        }
    }

    if let Some((_, key)) = key_opt {
        attrs.push((key, None));
    }
    if value_opt.is_some() {
        return Err(format!("Unexpected end of tag. Expected `'` or `\"`"));
    }

    Ok(attrs)
}

fn parse_tag_attribute_value(string: &str) -> Result<String, String> {
    // TODO: Replace escape characters
    Ok(string.to_string())
}
