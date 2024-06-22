# !/bin/bash
# Test - legacy completions API
curl http://127.0.0.1:8000/v1/completions -H "Content-Type: application/json" -H "Authorization: Bearer $OPENAI_API_KEY" -d '{
    "model": "gpt-3.5-turbo-instruct",
    "prompt": "Say this is a test",
    "max_tokens": 7,
    "temperature": 0
  }'

curl http://127.0.0.1:8000/v1/completions -H "Content-Type: application/json" -H "Authorization: Bearer $OPENAI_API_KEY" -d '{
    "model": "gpt-3.5-turbo-instruct",
    "prompt": "Tell Say th a joke",
    "n": 3,
    "max_tokens": 1024,
    "temperature": 0
  }'

curl -o /dev/null -s -w "\
Time to resolve DNS: %{time_namelookup}\n\
Time to connect: %{time_connect}\n\
Time to establish SSL: %{time_appconnect}\n\
Time to send request: %{time_pretransfer}\n\
Time to transfer start: %{time_starttransfer}\n\
Total Time: %{time_total}\n" http://127.0.0.1:8000/v1/completions -H "Content-Type: application/json" -H "Authorization: Bearer $OPENAI_API_KEY" -d '{
    "model": "gpt-3.5-turbo-instruct",
    "prompt": "Say this is a test",
    "max_tokens": 7,
    "temperature": 0
  }'

# Test - Chat completion
curl http://127.0.0.1:8000/v1/chat/completions \
	-H "Content-Type: application/json" \
	-H "Authorization: Bearer $OPENAI_API_KEY" \
	-d '{
    "model": "gpt-4o",
    "messages": [
      {
        "role": "system",
        "content": "You are a helpful assistant."
      },
      {
        "role": "user",
        "content": "Hello!"
      }
    ]
  }'
