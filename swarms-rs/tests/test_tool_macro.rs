//! Regression tests for the `#[tool]` macro's generated schemas and signatures.

use serde_json::{Value, json};
use swarms_macro::tool;
use swarms_rs::agent::TaskEvaluator;
use swarms_rs::structs::tool::ToolDyn;

#[derive(Debug, thiserror::Error)]
#[error("tool error")]
pub struct TestError;

fn params(tool: &impl ToolDyn) -> Value {
    tool.definition().parameters
}

fn required(tool: &impl ToolDyn) -> Vec<String> {
    params(tool)["required"]
        .as_array()
        .map(|names| {
            names
                .iter()
                .map(|n| n.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tool(arg(unit, description = "Temperature unit"))]
pub fn optional_arg(city: String, unit: Option<String>) -> Result<String, TestError> {
    Ok(format!("{city} {unit:?}"))
}

#[tool(arg(unit, description = "Temperature unit", required = true))]
pub fn forced_required(city: String, unit: Option<String>) -> Result<String, TestError> {
    Ok(format!("{city} {unit:?}"))
}

#[tool]
pub fn primitives(
    count: usize,
    offset: i64,
    big: u128,
    ratio: f64,
    initial: char,
    flag: bool,
    ids: Vec<u32>,
) -> Result<String, TestError> {
    Ok(format!(
        "{count} {offset} {big} {ratio} {initial} {flag} {ids:?}"
    ))
}

#[tool(arg(r#type, description = "Kind of item"))]
pub fn find_item(query: String, r#type: String) -> Result<String, TestError> {
    Ok(format!("{query}:{}", r#type))
}

#[tool]
pub fn increment(mut x: i32) -> std::result::Result<i32, TestError> {
    x += 1;
    Ok(x)
}

#[tool(name = "get-weather")]
pub fn get_weather(city: String) -> Result<String, TestError> {
    Ok(city)
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchArgs {
    query: String,
}

#[tool]
pub fn search(args: SearchArgs) -> Result<String, TestError> {
    Ok(args.query)
}

#[test]
fn option_args_use_inner_type_and_are_not_required() {
    let p = params(&OptionalArgTool);
    assert_eq!(p["properties"]["unit"]["type"], "string");
    assert_eq!(required(&OptionalArgTool), vec!["city"]);
}

#[test]
fn explicit_required_is_honored_for_option_args() {
    assert_eq!(required(&ForcedRequiredTool), vec!["city", "unit"]);
}

#[tokio::test]
async fn task_evaluator_context_is_an_optional_string() {
    let p = params(&TaskEvaluator);
    assert_eq!(p["properties"]["context"]["type"], "string");
    assert_eq!(required(&TaskEvaluator), vec!["status"]);

    let result = ToolDyn::call(&TaskEvaluator, r#"{"status":"Complete"}"#.to_string()).await;
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn primitive_json_types() {
    let props = &params(&PrimitivesTool)["properties"];
    assert_eq!(props["count"]["type"], "integer");
    assert_eq!(props["offset"]["type"], "integer");
    assert_eq!(props["big"]["type"], "integer");
    assert_eq!(props["ratio"]["type"], "number");
    assert_eq!(props["initial"]["type"], "string");
    assert_eq!(props["flag"]["type"], "boolean");
    assert_eq!(props["ids"]["type"], "array");
    assert_eq!(props["ids"]["items"]["type"], "integer");
}

#[tokio::test]
async fn raw_identifier_args_use_unraw_names() {
    let p = params(&FindItemTool);
    assert!(p["properties"].get("type").is_some(), "{p}");
    assert!(p["properties"].get("r#type").is_none(), "{p}");
    assert_eq!(required(&FindItemTool), vec!["query", "type"]);

    let args = json!({"query": "a", "type": "b"}).to_string();
    let result = ToolDyn::call(&FindItemTool, args).await.unwrap();
    assert_eq!(result, "\"a:b\"");
}

#[tokio::test]
async fn mut_args_and_qualified_result_compile_and_run() {
    let result = ToolDyn::call(&IncrementTool, r#"{"x":41}"#.to_string())
        .await
        .unwrap();
    assert_eq!(result, "42");
}

#[test]
fn hyphenated_tool_name_is_kept_as_advertised() {
    assert_eq!(ToolDyn::name(&GetWeatherTool), "get-weather");
    assert_eq!(GetWeatherTool.definition().name, "get-weather");
}

#[tokio::test]
async fn arg_type_named_like_generated_args_struct() {
    let args = json!({"args": {"query": "rust"}}).to_string();
    let result = ToolDyn::call(&SearchTool, args).await.unwrap();
    assert_eq!(result, "\"rust\"");
}
