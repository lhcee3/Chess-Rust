use crate::game::Game;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

pub async fn get_ai_move(game: &Game, api_key: &str) -> Result<String, String> {
    let fen = game.to_fen();
    let legal_moves = game.get_legal_moves();
    let legal_ucis: Vec<String> = legal_moves.iter().map(|m| game.move_to_uci(m)).collect();
    let model = &game.groq_model;

    let prompt = format!(
        "You are a chess engine. Current position FEN: {fen}\n\
         You play {side}. Legal moves: {moves}\n\n\
         Analyze the position and respond with ONLY:\n\
         bestmove <your_move_in_uci>\n\n\
         Examples: bestmove e2e4, bestmove g8f6, bestmove e7e8q\n\
         Choose from the legal moves listed above.",
        fen = fen,
        side = if game.side_to_move == crate::board::Color::White { "white" } else { "black" },
        moves = legal_ucis.join(", "),
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "system",
                "content": "You are a UCI chess engine. Only output 'bestmove <uci>' or 'bestmove <uci> ponder <uci>'."
            },
            {
                "role": "user",
                "content": prompt
            }
        ],
        "temperature": 0.2,
        "max_tokens": 50,
    });

    let client = reqwest::Client::new();
    let response = client
        .post(GROQ_API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("HTTP request failed: {}", e))?;

    let status = response.status();
    let text = response.text().await.map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(format!("Groq API error ({}): {}", status, text));
    }

    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse JSON response: {} - body: {}", e, text))?;

    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| format!("No content in response: {}", text))?;

    let content = content.trim();

    let uci = if let Some(idx) = content.find("bestmove") {
        let after = &content[idx + 8..];
        after.split_whitespace().next().unwrap_or("").to_string()
    } else {
        content.split_whitespace().next().unwrap_or("").to_string()
    };

    if uci.len() < 4 {
        return Err(format!("AI returned invalid move format: '{}'", content));
    }

    if legal_ucis.contains(&uci) {
        Ok(uci)
    } else {
        Err(format!(
            "AI returned illegal move '{}' (legal: {})",
            uci, legal_ucis.join(", ")
        ))
    }
}

pub async fn get_ai_move_with_retry(game: &Game, api_key: &str, max_retries: u32) -> Result<String, String> {
    let mut last_error = String::new();
    for attempt in 0..max_retries {
        match get_ai_move(game, api_key).await {
            Ok(uci) => return Ok(uci),
            Err(e) => {
                last_error = e;
                if attempt < max_retries - 1 {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
    }
    Err(format!("AI failed after {} retries: {}", max_retries, last_error))
}
