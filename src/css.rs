// This is not a rigorous or complete CSS serializer. But it's good enough for project needs.

#[derive(Clone, Debug)]
pub enum CssBlockContents {
    Line(String),
    Block(CssBlock),
}

impl CssBlockContents {
    pub fn line<S: Into<String>>(contents: S) -> Self {
        Self::Line(contents.into())
    }

    // pub fn block(contents: CssBlock) -> Self {
    //     Self::Block(contents)
    // }
}

#[derive(Clone, Copy, Debug)]
enum CssBlockMostRecentContents {
    None,
    Line,
    Block,
}

#[derive(Clone, Debug)]
pub struct CssBlock {
    pub prefix: String,
    pub contents: Vec<CssBlockContents>,
}

impl CssBlock {
    pub const fn empty() -> Self {
        Self {
            prefix: String::new(),
            contents: Vec::new(),
        }
    }

    pub fn new<S: Into<String>>(prefix: S, contents: Vec<CssBlockContents>) -> Self {
        Self {
            prefix: prefix.into(),
            contents,
        }
    }

    fn is_empty(&self) -> bool {
        match self.contents.is_empty() {
            true => true,
            false => self.contents.iter().any(|contents| match contents {
                CssBlockContents::Line(_) => false,
                CssBlockContents::Block(block) => !block.is_empty(),
            }),
        }
    }

    fn to_string(&self, indentation: usize) -> Option<String> {
        match self.is_empty() {
            true => None,
            false => {
                let wrapper_tabs = "\t".repeat(indentation);
                let contents_tabs = "\t".repeat(indentation + 1);

                let mut last_contents = CssBlockMostRecentContents::None;

                let mut output = format!("{wrapper_tabs}{} {{", self.prefix);
                for contents in &self.contents {
                    match contents {
                        CssBlockContents::Line(line_string) => {
                            let initial_newlines = match last_contents {
                                CssBlockMostRecentContents::Block => "\n\n",
                                _ => "\n",
                            };
                            output.push_str(&format!(
                                "{initial_newlines}{contents_tabs}{line_string}"
                            ));
                            last_contents = CssBlockMostRecentContents::Line;
                        }
                        CssBlockContents::Block(block) => {
                            if let Some(block_string) = block.to_string(indentation + 1) {
                                let initial_newlines = match last_contents {
                                    CssBlockMostRecentContents::None => "\n",
                                    _ => "\n\n",
                                };
                                output.push_str(&format!("{initial_newlines}{block_string}"));
                                last_contents = CssBlockMostRecentContents::Block;
                            }
                        }
                    }
                }
                output.push_str(&format!("\n{wrapper_tabs}}}"));

                Some(output)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CssFile {
    pub blocks: Vec<CssBlock>,
}

impl CssFile {
    pub const fn new(blocks: Vec<CssBlock>) -> Self {
        Self { blocks }
    }

    pub fn to_string(&self) -> Option<String> {
        let nonempty_block_strings = self
            .blocks
            .iter()
            .filter_map(|block| block.to_string(0))
            .collect::<Vec<_>>();
        match nonempty_block_strings.is_empty() {
            true => None,
            false => Some(format!("{}\n", nonempty_block_strings.join("\n\n"))),
        }
    }
}
