use std::sync::Arc;

use crate::diagnostics::LspError;
use crate::utils::apply_document_changes;
use crate::{documents::Document, session::Session};
use biome_fs::BiomePath;
use biome_service::workspace::{
    ChangeFileParams, CloseFileParams, DocumentFileSource, FileContent, GetFileContentParams,
    OpenFileParams, OpenProjectParams,
};
use tower_lsp_server::lsp_types;
use tracing::{debug, error, field, info};

/// Handler for `textDocument/didOpen` LSP notification
#[tracing::instrument(
    level = "debug",
    skip_all,
    fields(
        text_document_uri = display(params.text_document.uri.as_str()),
        text_document_language_id = display(&params.text_document.language_id),
    )
)]
pub(crate) async fn did_open(
    session: &Arc<Session>,
    params: lsp_types::DidOpenTextDocumentParams,
) -> Result<(), LspError> {
    let url = params.text_document.uri;
    let version = params.text_document.version;
    let content = params.text_document.text;
    let language_hint = DocumentFileSource::from_language_id(&params.text_document.language_id);

    let path = session.file_path(&url)?;
    let project_key = match session.project_for_path(&path) {
        Some(project_key) => project_key,
        None => {
            info!("No open project for path: {path:?}. Opening new project.");
            let parent_path = BiomePath::new(
                path.parent()
                    .map(|parent| parent.to_path_buf())
                    .unwrap_or_default(),
            );
            let project_key = session.workspace.open_project(OpenProjectParams {
                path: parent_path.clone(),
                open_uninitialized: true,
            })?;
            session.insert_and_scan_project(project_key, parent_path);
            project_key
        }
    };

    let doc = Document::new(project_key, version, &content);

    session.workspace.open_file(OpenFileParams {
        project_key,
        path,
        content: FileContent::FromClient { content, version },
        document_file_source: Some(language_hint),
        persist_node_cache: true,
    })?;

    session.insert_document(url.clone(), doc);

    if let Err(err) = session.update_diagnostics(url).await {
        error!("Failed to update diagnostics: {}", err);
    }

    Ok(())
}

/// Handler for `textDocument/didChange` LSP notification
#[tracing::instrument(level = "debug", skip_all, fields(url = field::display(&params.text_document.uri.as_str()), version = params.text_document.version), err)]
pub(crate) async fn did_change(
    session: &Session,
    params: lsp_types::DidChangeTextDocumentParams,
) -> Result<(), LspError> {
    let url = params.text_document.uri;
    let version = params.text_document.version;

    let path = session.file_path(&url)?;
    let doc = session.document(&url)?;

    let old_text = session.workspace.get_file_content(GetFileContentParams {
        project_key: doc.project_key,
        path: path.clone(),
    })?;
    debug!("old document: {:?}", old_text);
    debug!("content changes: {:?}", params.content_changes);

    let text = apply_document_changes(
        session.position_encoding(),
        old_text,
        params.content_changes,
    );

    debug!("new document: {:?}", text);

    session.insert_document(url.clone(), Document::new(doc.project_key, version, &text));

    session.workspace.change_file(ChangeFileParams {
        project_key: doc.project_key,
        path,
        version,
        content: text,
    })?;

    if let Err(err) = session.update_diagnostics(url).await {
        error!("Failed to update diagnostics: {}", err);
    }

    Ok(())
}

/// Handler for `textDocument/didClose` LSP notification
#[tracing::instrument(level = "debug", skip(session), err)]
pub(crate) async fn did_close(
    session: &Session,
    params: lsp_types::DidCloseTextDocumentParams,
) -> Result<(), LspError> {
    let uri = params.text_document.uri;
    let path = session.file_path(&uri)?;
    let Some(project_key) = session.remove_document(&uri) else {
        debug!("Document wasn't open: {}", uri.as_str());
        return Ok(());
    };

    session
        .workspace
        .close_file(CloseFileParams { project_key, path })?;

    session
        .client
        .publish_diagnostics(uri, Vec::new(), None)
        .await;

    Ok(())
}
