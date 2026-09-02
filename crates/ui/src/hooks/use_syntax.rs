//! Syntax state and requests for rendered diff content.

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use file_types::{DiffType, DiffVersion, File};
use loom::{Scope, use_effect, use_state};
use pipeline::diff::DiffContent;
use syntax::Store;

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

pub fn use_syntax(
    scope: &mut Scope,
    syntax_service: Option<Rc<SyntaxService>>,
    content: Rc<DiffContent>,
    diff_type: DiffType,
    visible_lines: Range<u32>,
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
    visible_lines: Range<u32>,
) -> Vec<SyntaxRequest> {
    match content {
        DiffContent::Diff(diff) if diff_type != DiffType::Single => {
            let visible: Vec<_> = diff
                .alignment
                .view_lines_from(diff_type, visible_lines.start)
                .take(visible_lines.len())
                .collect();
            let mut requests = Vec::with_capacity(2);
            let original_last = visible
                .iter()
                .filter_map(|line| line.original.line())
                .max()
                .and_then(|line| line.checked_sub(1));
            if let Some(last) = original_last {
                requests.push(SyntaxRequest {
                    file: diff.file.clone(),
                    version: DiffVersion::Original,
                    text: diff.alignment.text(DiffVersion::Original),
                    last,
                });
            }
            let modified_last = visible
                .iter()
                .filter_map(|line| line.modified.line())
                .max()
                .and_then(|line| line.checked_sub(1));
            if let Some(last) = modified_last {
                requests.push(SyntaxRequest {
                    file: diff.file.clone(),
                    version: DiffVersion::Modified,
                    text: diff.alignment.text(DiffVersion::Modified),
                    last,
                });
            }
            requests
        }
        DiffContent::SingleFile(single) if diff_type == DiffType::Single => {
            let line_count = single.lines.len() as u32;
            if visible_lines.start >= line_count {
                return Vec::new();
            }
            let Some(last) = visible_lines.end.min(line_count).checked_sub(1) else {
                return Vec::new();
            };
            vec![SyntaxRequest {
                file: single.file.clone(),
                version: single.side(),
                text: Arc::clone(&single.lines),
                last,
            }]
        }
        _ => Vec::new(),
    }
}
