import os
from openai import OpenAI

# gets API Key from environment variable OPENAI_API_KEY
client = OpenAI(
    api_key=os.environ.get("OPENAI_API_KEY"),

    # default is https://api.openai.com/v1/
    # base url for testing locally
    # base_url="http://127.0.0.1:8000/v1/"
    # base url for production instance
    # base_url = "https://felafax-proxy.shuttleapp.rs/v1/"
    base_url = "https://felafax-proxy-sq5shdnepa-uc.a.run.app/v1/"

)

# Non-streaming:
print("----- standard request -----")
completion = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {
            "role": "user",
            "content": "Say this is a test",
        },
    ],
)
print(completion.choices[0].message.content)

# Streaming:
print("----- streaming request -----")
# stream = client.chat.completions.create(
#     model="gpt-4o",
#     messages=[
#         {
#             "role": "user",
#             "content": "How do I output all files in a directory using Python?",
#         },
#     ],
#     stream=True,
# )
stream = client.chat.completions.create(
    model="gpt-4o",
    messages=[
        {
            "role": "user",
            "content": "Tell me a joke",
        },
    ],
    stream=True,
)
for chunk in stream:
    if not chunk.choices:
        continue

    print(chunk.choices[0].delta.content, end="")
print()

# Response headers:
print("----- custom response headers test -----")
response = client.chat.completions.with_raw_response.create(
    model="gpt-4o",
    messages=[
        {
            "role": "user",
            "content": "Say this is a test",
        }
    ],
)
completion = response.parse()
print(response.request_id)
print(completion.choices[0].message.content)
