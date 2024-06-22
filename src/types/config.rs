pub trait LLMConfig {
    fn get_api_key(&self) -> String;

    fn set_api_key(&mut self, api_key: &str);

    fn get_base_url(&self) -> String;

    fn get_name(&self) -> String;

    fn get_default_model(&self) -> String;
}
