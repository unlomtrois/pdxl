use pdxl_mcp::docs::{self, GetEntityDocsParams};
use pdxl_mcp::{SearchScriptItemsParams, search_script_items};
use rmcp::{
    ErrorData as McpError, Json, RoleServer, ServerHandler, ServiceExt,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{
        AnnotateAble, ListResourcesResult, PaginatedRequestParams, RawResource,
        ReadResourceRequestParams, ReadResourceResult, ResourceContents, ServerCapabilities,
        ServerInfo,
    },
    service::RequestContext,
    tool, tool_handler, tool_router,
    transport::stdio,
};

const DOCS_URI_PREFIX: &str = "docs://";

#[derive(Clone)]
struct PdxlMcp {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl PdxlMcp {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Search the compiled-in game's built-in effects, triggers, modifiers, and scope links by key or documentation text. The game is selected by the ck3 or eu5 build feature. Use input_scope to return only items legal from a known scope. Results are deterministically ranked and capped."
    )]
    fn search_script_items(
        &self,
        Parameters(params): Parameters<SearchScriptItemsParams>,
    ) -> Json<pdxl_mcp::SearchScriptItemsResult> {
        Json(search_script_items(params))
    }

    #[tool(
        description = "Curated knowledge base for one of the compiled-in game's script systems (e.g. \"activities\"): how its databases reference each other, where the game's own .info docs and the shipped files disagree, unwritten conventions, pitfalls, and a working skeleton. Omit `entity` to list what is available. The same documents are served as docs:// resources."
    )]
    fn get_entity_docs(
        &self,
        Parameters(params): Parameters<GetEntityDocsParams>,
    ) -> Json<pdxl_mcp::docs::GetEntityDocsResult> {
        Json(docs::get_entity_docs(params))
    }
}

#[tool_handler]
impl ServerHandler for PdxlMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(format!(
                "Query {} script documentation and scope-aware built-in metadata. This MCP binary contains only the game selected at build time. Curated per-system knowledge bases are available as docs:// resources and through the get_entity_docs tool.",
                pdxl_game::GAME.to_uppercase()
            )),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
            ..Default::default()
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = docs::ENTITY_DOCS
            .iter()
            .map(|doc| {
                let mut resource =
                    RawResource::new(format!("{DOCS_URI_PREFIX}{}", doc.name), doc.name);
                resource.title = Some(doc.title.to_string());
                resource.description = Some(doc.summary.to_string());
                resource.mime_type = Some("text/markdown".to_string());
                resource.no_annotation()
            })
            .collect();
        Ok(ListResourcesResult {
            resources,
            ..Default::default()
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let name = request
            .uri
            .strip_prefix(DOCS_URI_PREFIX)
            .ok_or_else(|| McpError::resource_not_found(request.uri.clone(), None))?;
        let doc = docs::find(name)
            .ok_or_else(|| McpError::resource_not_found(request.uri.clone(), None))?;
        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: request.uri,
                mime_type: Some("text/markdown".to_string()),
                text: doc.markdown.to_string(),
                meta: None,
            }],
        })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = PdxlMcp::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
