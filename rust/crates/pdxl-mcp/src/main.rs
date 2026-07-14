use pdxl_mcp::{SearchScriptItemsParams, search_script_items};
use rmcp::{
    Json, ServerHandler, ServiceExt,
    handler::server::tool::ToolRouter,
    handler::server::wrapper::Parameters,
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
};

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
        description = "Search CK3 built-in effects, triggers, modifiers, and scope links by key or documentation text. Use input_scope to return only items legal from a known scope. Results are deterministically ranked and capped."
    )]
    fn search_script_items(
        &self,
        Parameters(params): Parameters<SearchScriptItemsParams>,
    ) -> Json<pdxl_mcp::SearchScriptItemsResult> {
        Json(search_script_items(params))
    }
}

#[tool_handler]
impl ServerHandler for PdxlMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Query CK3 script documentation and scope-aware built-in metadata.".into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = PdxlMcp::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
