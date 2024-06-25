import subprocess
import json
import statistics

# Define the command templates
commands = {
    "OpenAI": [
        "curl", "-o", "/dev/null", "-s", "-w",
        "\
Time to resolve DNS: %{time_namelookup}\n\
Time to connect: %{time_connect}\n\
Time to establish SSL: %{time_appconnect}\n\
Time to send request: %{time_pretransfer}\n\
Time to transfer start: %{time_starttransfer}\n\
Total Time: %{time_total}\n",
        "http://127.0.0.1:8000/v1/chat/completions",
        "-H", "Content-Type: application/json",
        "-H", f"Authorization: Bearer $OPENAI_API_KEY",
        "-d", json.dumps({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ]
        })
    ],
    "Felafax": [
        "curl", "-o", "/dev/null", "-s", "-w",
        "\
Time to resolve DNS: %{time_namelookup}\n\
Time to connect: %{time_connect}\n\
Time to establish SSL: %{time_appconnect}\n\
Time to send request: %{time_pretransfer}\n\
Time to transfer start: %{time_starttransfer}\n\
Total Time: %{time_total}\n",
        "http://felafax-proxy.shuttleapp.rs/v1/chat/completions",
        "-H", "Content-Type: application/json",
        "-H", f"Authorization: Bearer $OPENAI_API_KEY",
        "-d", json.dumps({
            "model": "gpt-4o",
            "messages": [
                {"role": "system", "content": "You are a helpful assistant."},
                {"role": "user", "content": "Hello!"}
            ]
        })
    ]
}

# Function to run the command and collect timing data
def run_command(command):
    result = subprocess.run(command, capture_output=True, text=True)
    output = result.stdout
    times = {
        "dns": float(output.split("Time to resolve DNS: ")[1].split("\n")[0]),
        "connect": float(output.split("Time to connect: ")[1].split("\n")[0]),
        "ssl": float(output.split("Time to establish SSL: ")[1].split("\n")[0]),
        "pretransfer": float(output.split("Time to send request: ")[1].split("\n")[0]),
        "starttransfer": float(output.split("Time to transfer start: ")[1].split("\n")[0]),
        "total": float(output.split("Total Time: ")[1].split("\n")[0])
    }
    return times

# Collect data
data = {"OpenAI": [], "Felafax": []}
for i in range(10):
    print(f"Iteration {i+1}")
    for service in ["OpenAI", "Felafax"]:
        times = run_command(commands[service])
        data[service].append(times)

# Calculate averages
averages = {service: {key: statistics.mean([run[key] for run in runs]) for key in runs[0]} for service, runs in data.items()}

# Calculate differences
differences = {key: (averages["OpenAI"][key] - averages["Felafax"][key]) * 1000 for key in averages["OpenAI"]}

# Print results
print("\nAverages (in seconds):")
print(f"{'Metric':<20}{'OpenAI':<15}{'Felafax':<15}{'Difference (ms)':<20}")
for key in averages["OpenAI"]:
    print(f"{key.capitalize():<20}{averages['OpenAI'][key]:<15.3f}{averages['Felafax'][key]:<15.3f}{differences[key]:<20.3f}")

