import requests
import time
import statistics
from typing import List, Dict
import os
import csv
from datetime import datetime

def make_api_request(url: str, headers: Dict[str, str], data: Dict) -> Dict[str, float]:
    start_time = time.time()
    
    response = requests.post(url, headers=headers, json=data, stream=True)
    
    first_byte_time = None
    content = b""
    chunk_times = []
    
    for chunk in response.iter_content(chunk_size=1):
        if first_byte_time is None:
            first_byte_time = time.time()
        content += chunk
        chunk_times.append(time.time())
    
    end_time = time.time()
    
    total_time = end_time - start_time
    ttfb = first_byte_time - start_time if first_byte_time else None
    request_to_response_time = response.elapsed.total_seconds()
    response_size = len(content)
    transfer_rate = response_size / total_time if total_time > 0 else 0
    
    avg_time_between_chunks = statistics.mean([chunk_times[i+1] - chunk_times[i] for i in range(len(chunk_times)-1)]) if len(chunk_times) > 1 else 0
    
    return {
        "total_time": total_time,
        "ttfb": ttfb,
        "request_to_response_time": request_to_response_time,
        "response_size": response_size,
        "transfer_rate": transfer_rate,
        "status_code": response.status_code,
        "chunk_count": len(chunk_times),
        "avg_time_between_chunks": avg_time_between_chunks
    }

def calculate_stats(data: List[float]) -> Dict[str, float]:
    return {
        "avg": statistics.mean(data),
        "p75": statistics.quantiles(data, n=4)[2],
        "p95": statistics.quantiles(data, n=20)[18]
    }

def format_value(value):
    if isinstance(value, float):
        return f"{value:.2f}"
    return str(value)

def print_markdown_table(openai_results, felafax_results):
    metrics = [
        "total_time", "ttfb", "request_to_response_time", "response_size",
        "transfer_rate", "chunk_count", "avg_time_between_chunks"
    ]

    print("| Metric | OpenAI API | Felafax API |")
    print("|--------|------------|-------------|")

    for metric in metrics:
        openai_data = [result[metric] for result in openai_results if result[metric] is not None]
        felafax_data = [result[metric] for result in felafax_results if result[metric] is not None]

        if openai_data and felafax_data:
            openai_stats = calculate_stats(openai_data)
            felafax_stats = calculate_stats(felafax_data)

            print(f"| {metric.capitalize()} (avg) | {format_value(openai_stats['avg'])} | {format_value(felafax_stats['avg'])} |")
            print(f"| {metric.capitalize()} (p75) | {format_value(openai_stats['p75'])} | {format_value(felafax_stats['p75'])} |")
            print(f"| {metric.capitalize()} (p95) | {format_value(openai_stats['p95'])} | {format_value(felafax_stats['p95'])} |")

    openai_status_codes = set(result["status_code"] for result in openai_results if result["status_code"] is not None)
    felafax_status_codes = set(result["status_code"] for result in felafax_results if result["status_code"] is not None)

    print(f"| Status Codes | {', '.join(map(str, openai_status_codes))} | {', '.join(map(str, felafax_status_codes))} |")

def write_csv(openai_results, felafax_results, filename):
    metrics = [
        "total_time", "ttfb", "request_to_response_time", "response_size",
        "transfer_rate", "chunk_count", "avg_time_between_chunks", "status_code"
    ]

    with open(filename, 'w', newline='') as csvfile:
        writer = csv.writer(csvfile)
        writer.writerow(['API', 'Test Number'] + metrics)

        for i, (openai_result, felafax_result) in enumerate(zip(openai_results, felafax_results), 1):
            writer.writerow(['OpenAI', i] + [openai_result.get(metric) for metric in metrics])
            writer.writerow(['Felafax', i] + [felafax_result.get(metric) for metric in metrics])

def main():
    openai_api_key = os.environ.get('OPENAI_API_KEY')
    if not openai_api_key:
        raise ValueError("OPENAI_API_KEY environment variable is not set")

    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {openai_api_key}"
    }

    data = {
        "model": "gpt-4o",
        "messages": [
            {
                "role": "system",
                "content": "You are great poem writer"
            },
            {
                "role": "user",
                "content": "Write a 10 line haiku about felafax"
            }
        ],
        "stream": True
    }

    openai_url = "https://api.openai.com/v1/chat/completions"
    felafax_url = "https://felafax-proxy-sq5shdnepa-uc.a.run.app/v1/chat/completions"

    openai_results = []
    felafax_results = []

    print("Running tests...")
    runs = 20
    for i in range(runs):
        print(f"Test {i+1}/{runs}")
        openai_results.append(make_api_request(openai_url, headers, data))
        felafax_results.append(make_api_request(felafax_url, headers, data))

    print("\nResults:")
    print_markdown_table(openai_results, felafax_results)

    # Generate a unique filename with timestamp
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    csv_filename = f"api_performance_comparison_{timestamp}.csv"

    write_csv(openai_results, felafax_results, csv_filename)
    print(f"\nDetailed results have been written to {csv_filename}")

if __name__ == "__main__":
    main()
