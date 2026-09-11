//! Syntax state and requests for rendered diff content.

use std::rc::Rc;
use std::sync::Arc;

use file_types::{DiffType, DiffVersion, File};
use loom::{Scope, use_effect, use_state};
use pipeline::diff::DiffContent;
use syntax::Store;

use crate::components::WrappedViewLine;
use crate::services::syntax::SyntaxService;

#[derive(Clone)]
struct SyntaxRequest {
    file: File,
    version: DiffVersion,
    text: Arc<Vec<String>>,
    last: u32,
}

impl PartialEq for SyntaxRequest {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
            && self.version == other.version
            && Arc::ptr_eq(&self.text, &other.text)
            && self.last == other.last
    }
}

pub(crate) fn use_syntax(
    scope: &mut Scope,
    syntax_service: Option<Rc<SyntaxService>>,
    content: Rc<DiffContent>,
    diff_type: DiffType,
    visible_lines: &[WrappedViewLine],
) -> Option<Rc<Store>> {
    let content_id = Rc::as_ptr(&content) as usize;
    let syntax_service_id = syntax_service
        .as_ref()
        .map(|syntax_service| Rc::as_ptr(syntax_service) as usize);
    let (syntax, set_syntax) = use_state(scope, || None::<(usize, Rc<Store>)>);

    let syntax_service_for_subscription = syntax_service.as_ref().map(Rc::clone);
    use_effect(scope, (content_id, syntax_service_id), move || {
        set_syntax(&|_| None);
        let Some(syntax_service) = syntax_service_for_subscription else {
            return;
        };
        syntax_service.new_file();
        syntax_service.subscribe().subscribe(move |store| {
            set_syntax(&move |_| Some((content_id, Rc::clone(&store))));
        });
    });

    let requests = syntax_requests(&content, diff_type, visible_lines);
    let syntax_service_for_requests = syntax_service;
    let requests_for_effect = requests.clone();
    use_effect(
        scope,
        (content_id, syntax_service_id, requests),
        move || {
            let Some(syntax_service) = syntax_service_for_requests else {
                return;
            };
            for request in requests_for_effect {
                syntax_service.request(&request.file, request.version, request.text, request.last);
            }
        },
    );

    syntax
        .filter(|(syntax_content_id, _)| *syntax_content_id == content_id)
        .map(|(_, store)| store)
}

fn syntax_requests(
    content: &DiffContent,
    diff_type: DiffType,
    visible_lines: &[WrappedViewLine],
) -> Vec<SyntaxRequest> {
    match content {
        DiffContent::Diff(diff) if diff_type != DiffType::Single => {
            let mut requests = Vec::with_capacity(2);
            for version in [DiffVersion::Original, DiffVersion::Modified] {
                let Some(last) = last_source_line(visible_lines, version) else {
                    continue;
                };
                requests.push(SyntaxRequest {
                    file: diff.file.clone(),
                    version,
                    text: diff.alignment.text(version),
                    last,
                });
            }
            requests
        }
        DiffContent::SingleFile(single) if diff_type == DiffType::Single => {
            let version = single.side();
            last_source_line(visible_lines, version)
                .map(|last| {
                    vec![SyntaxRequest {
                        file: single.file.clone(),
                        version,
                        text: Arc::clone(&single.lines),
                        last,
                    }]
                })
                .unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

fn last_source_line(visible_lines: &[WrappedViewLine], version: DiffVersion) -> Option<u32> {
    let mut last_line: Option<u32> = None;
    for line in visible_lines {
        let terminal_lines = match version {
            DiffVersion::Original => &line.original,
            DiffVersion::Modified => &line.modified,
        };
        for terminal_line in terminal_lines {
            if let Some(source_line) = terminal_line.source_line() {
                last_line = Some(last_line.map_or(source_line, |last| last.max(source_line)));
            }
        }
    }
    last_line.and_then(|line| line.checked_sub(1))
}
