# Felafax Gateway
website: https://felafax.dev/

## Overview
`felafax-gateway` is a fast and lightweight proxy for LLMs, written in Rust. Designed for low latency and high scalability, it operates in two modes:

1. **Proxy Mode**: Proxies requests to the OpenAI API, supporting various experimentation and roll-out features.
2. **Translate Mode**: Translates requests from other LLMs, like Claude and Jamba, into the OpenAI spec, enabling easy adoption of new LLMs.

## Goals
* Enable dynamic replacement of "system prompts" for experimentation and roll-out.
* Support all OpenAI APIs through the proxy.
* Translate all LLM APIs into the OpenAI spec, enabling easy adoption of new LLMs.
* Achieve low latency and high scalability, making it ideal for production use cases.
* Support hot-swapping of LLMs without requiring code changes.

## Usage

### Proxy Mode
```py
import os
import json
from openai import OpenAI

client = OpenAI(
    # default is https://api.openai.com/v1/
    base_url = "https://openai.felafax.ai/v1"
)


chat_completion = client.chat.completions.create(
    # continue with OpenAI uscase
)
```
### Translate Mode
```py
import os
from openai import OpenAI

client = OpenAI(
    # Pass in felafax api key from portal
    api_key=os.environ.get("FELAFAX_API_KEY"),
    # Update the url to point to proxy
    base_url = "https://openai.felafax.ai/translate/v1"

)

completion = client.chat.completions.create(
    messages=[
        {
            "role": "user",
            "content": "Say this is a test",
        }
    ],
    # Set this to hot-swap to switch between LLMs using config
    # model="hot-swap",
    # Or pass the model name directly and we'll convert it to the right LLM
    model="gpt-3.5-turbo",
    # model="claude-3-5-sonnet-20240620"
    # model="jamba-instruct-preview"
)

print(completion.choices[0].message.content)
```

## Run
```sh
# docker build
docker build -t felafax-proxy .
# docker run
docker run -p 8080:8080 -v $(pwd)/firebase.json:/firebase.json felafax-proxy
```

## Benchmarks
> Comparision between OpenAI API and Felafax API on 20 iterations.

| Metric                           | OpenAI API | Felafax API |
|----------------------------------|------------|-------------|
| Total_time (avg)                 | 1.36       | 1.53        |
| Total_time (p75)                 | 1.46       | 1.78        |
| Total_time (p95)                 | 2.39       | 2.25        |
| Ttfb (avg)                       | 0.54       | 0.69        |
| Ttfb (p75)                       | 0.63       | 0.79        |
| Ttfb (p95)                       | 1.12       | 1.35        |
| Request_to_response_time (avg)   | 0.54       | 0.68        |
| Request_to_response_time (p75)   | 0.63       | 0.79        |
| Request_to_response_time (p95)   | 1.12       | 1.35        |
| Response_size (avg)              | 17833.85   | 19346.70    |
| Response_size (p75)              | 20409.00   | 20991.00    |
| Response_size (p95)              | 23761.25   | 32694.05    |
| Transfer_rate (avg)              | 13585.13   | 12831.44    |
| Transfer_rate (p75)              | 15039.40   | 14864.51    |
| Transfer_rate (p95)              | 19186.19   | 17041.68    |
| Chunk_count (avg)                | 17833.85   | 19346.70    |
| Chunk_count (p75)                | 20409.00   | 20991.00    |
| Chunk_count (p95)                | 23761.25   | 32694.05    |
| Avg_time_between_chunks (avg)    | 0.00       | 0.00        |
| Avg_time_between_chunks (p75)    | 0.00       | 0.00        |
| Avg_time_between_chunks (p95)    | 0.00       | 0.00        |

## Supported Features
* We support proxy for all OpenAI APIs.
* We support `/chat/completions` for each of these LLMs.
* Supported LLMs
  - [x] OpenAI
  - [x] Claude
  - [x] Jamba

## Roadmap:
* [ ] Support configurable request log storage (S3, GCS, etc).
* [ ] Support streaming completion in translate mode.




