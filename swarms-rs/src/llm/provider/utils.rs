use super::any::ModelNameError;

pub(super) fn read_api_key(model: &str, var: &'static str) -> Result<String, ModelNameError> {
    std::env::var(var)
        .ok()
        .filter(|key| !key.is_empty())
        .ok_or_else(|| ModelNameError::MissingApiKey {
            model: model.to_string(),
            var,
        })
}
