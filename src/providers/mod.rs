pub mod google;

use async_trait::async_trait;

use crate::error::VigenError;

/// Abstraction over vision-capable AI providers.
#[async_trait]
pub trait VisionProvider: Send + Sync {
    /// Send an image + text prompt to a vision model and return its response.
    async fn analyze_image(
        &self,
        image_data: &[u8],
        mime_type: &str,
        prompt: &str,
    ) -> Result<String, VigenError>;
}
