use printpdf::*;
use std::fs::File;
use std::io::BufWriter;

pub struct AlgoPdfLogger {
    algo_id: String,
    algo_type: String,
    log_buffer: Vec<String>, // Buffer to store log messages in memory
}

impl AlgoPdfLogger {
    /// Creates a new buffered logger for the specified algorithm ID and type
    pub fn new(algo_id: &str, algo_type: &str) -> Self {
        Self {
            algo_id: algo_id.to_string(),
            algo_type: algo_type.to_string(),
            log_buffer: Vec::new(),
        }
    }

    pub fn log_message(&mut self, message: &str) {
        let timestamp = chrono::Local::now().format("%I:%M:%S %p").to_string();
        let formatted_message = format!("Execution report at {}: {}", timestamp, message);
        self.log_buffer.push(formatted_message);
    }

    pub fn write_to_pdf(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (doc, page, layer) = PdfDocument::new(
            format!("Algorithm Report: {}", self.algo_id),
            Mm(210.0),
            Mm(297.0),
            "Layer 1",
        );
        let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
        let font_size = 12.0;
        let line_height = Mm(5.0); // Distance between lines within the same entry
        let entry_spacing = Mm(10.0); // Larger spacing between different entries
        let max_chars_per_line = 75; // Maximum characters per line for better control

        let mut y_position = Mm(280.0); // Start near the top of the page
        let mut current_layer = doc.get_page(page).get_layer(layer);

        for message in &self.log_buffer {
            let lines = self.wrap_text(message, max_chars_per_line);

            for (i, line) in lines.iter().enumerate() {
                current_layer.use_text(line, font_size, Mm(10.0), y_position, &font);

                // Apply smaller spacing between wrapped lines within the same log entry
                y_position -= if i == lines.len() - 1 {
                    entry_spacing
                } else {
                    line_height
                };

                // Check if we need to add a new page
                if y_position < Mm(20.0) {
                    let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "Layer 1");
                    y_position = Mm(280.0);
                    current_layer = doc.get_page(new_page).get_layer(new_layer);
                    // Update the current_layer
                }
            }
        }

        // Save the document to a file with the format algoType_algoId_report
        let file_name = format!("logs/{}_{}_report.pdf", self.algo_type, self.algo_id);
        let file = File::create(file_name)?;
        doc.save(&mut BufWriter::new(file))?;

        Ok(())
    }

    /// Wraps text based on a character count per line
    fn wrap_text(&self, text: &str, max_chars_per_line: usize) -> Vec<String> {
        let mut wrapped_lines = Vec::new();
        let mut current_line = String::new();

        for word in text.split_whitespace() {
            // If adding the next word exceeds the character limit, push the current line and start a new one
            if current_line.len() + word.len() + 1 > max_chars_per_line {
                wrapped_lines.push(current_line);
                current_line = word.to_string();
            } else {
                if !current_line.is_empty() {
                    current_line.push(' ');
                }
                current_line.push_str(word);
            }
        }

        // Add any remaining text as the last line
        if !current_line.is_empty() {
            wrapped_lines.push(current_line);
        }

        wrapped_lines
    }
}

#[macro_export]
macro_rules! report {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log_message(&format!($($arg)*));
    };
}
