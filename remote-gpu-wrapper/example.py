#!/usr/bin/env python3
"""
Example usage of Remote GPU + NextPlaid wrapper.

This connects to a self-hosted NextPlaid API server running on a remote GPU machine.
"""

import os
import sys
import argparse
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

from remote_gpu_wrapper import RemoteGPUCodeSearch


def main():
    """Example: Index and search a codebase using remote GPU."""

    # Configuration
    SERVER_URL = os.getenv("REMOTE_GPU_URL", "http://localhost:8080")
    PROJECT_PATH = os.getenv("PROJECT_PATH", ".")
    INDEX_NAME = os.getenv("INDEX_NAME", "remote_project")

    print("=" * 60)
    print("Remote GPU + NextPlaid Code Search Example")
    print("=" * 60)
    print(f"Server: {SERVER_URL}")
    print(f"Index: {INDEX_NAME}")
    print(f"Project: {PROJECT_PATH}")
    print("-" * 60)

    # Initialize
    print("\n[1/4] Connecting to remote GPU server...")
    search = RemoteGPUCodeSearch(
        server_url=SERVER_URL,
        index_name=INDEX_NAME,
    )

    # Health check
    health = search.health_check()
    print(f"Server health: {health}")

    if health.get("status") != "healthy":
        print("ERROR: Cannot connect to remote server!")
        print("\nTroubleshooting:")
        print("  1. Is the Docker container running on the GPU server?")
        print("  2. Is the SSH tunnel active? (ssh -L 8080:localhost:8080 user@gpu-server)")
        print("  3. Check firewall settings on the GPU server")
        return

    # Show server info
    if health.get("gpu_available"):
        print("✓ GPU acceleration available on remote server")
    else:
        print("⚠ No GPU detected on remote server (CPU mode)")

    # Index project
    print(f"\n[2/4] Indexing project: {PROJECT_PATH}")
    print("(Parsing locally, sending text to remote GPU for encoding...)")

    num_indexed = search.index_project(
        PROJECT_PATH,
        exclude_dirs=['.git', 'node_modules', '__pycache__', '.venv', 'venv', 'target', 'dist', 'build'],
        batch_size=64,
    )

    if num_indexed == 0:
        print("No code units found! Check your PROJECT_PATH.")
        return

    # Show stats
    print("\n[3/4] Index statistics:")
    stats = search.get_stats()
    print(f"  Documents: {stats.get('num_documents', 0)}")
    print(f"  Embeddings: {stats.get('num_embeddings', 0)}")
    print(f"  Dimension: {stats.get('dimension', 0)}")

    # Search examples
    print("\n[4/4] Running example searches...")

    example_queries = [
        "database connection",
        "error handling",
        "authentication",
        "configuration parser",
        "http request",
    ]

    for query in example_queries:
        print(f"\nQuery: '{query}'")
        print("-" * 40)

        results = search.search(query, k=5)

        if not results:
            print("  (no results)")
            continue

        for i, result in enumerate(results[:3], 1):
            meta = result.get('metadata', {})
            file_path = meta.get('file', 'unknown')
            line = meta.get('line', 0)
            name = meta.get('name', 'unknown')
            unit_type = meta.get('unit_type', 'unknown')
            score = result.get('score', 0)

            print(f"  {i}. {file_path}:{line}")
            print(f"     Score: {score:.3f} | {name} ({unit_type})")

    print("\n" + "=" * 60)
    print("Done!")
    print("=" * 60)


def interactive_mode(server_url: str, index_name: str):
    """Interactive search mode."""
    print("=" * 60)
    print("Interactive Code Search (Remote GPU)")
    print("=" * 60)
    print(f"Server: {server_url}")
    print(f"Index: {index_name}")
    print("\nType 'quit' to exit, 'stats' for index info")
    print("-" * 60)

    search = RemoteGPUCodeSearch(
        server_url=server_url,
        index_name=index_name,
    )

    # Health check
    health = search.health_check()
    if health.get("status") != "healthy":
        print(f"ERROR: Cannot connect to server: {health}")
        return

    print(f"✓ Connected (GPU: {health.get('gpu_available', False)})")

    while True:
        try:
            query = input("\nSearch: ").strip()
        except (EOFError, KeyboardInterrupt):
            print("\nGoodbye!")
            break

        if query.lower() in ('quit', 'exit', 'q'):
            break
        if not query:
            continue

        if query.lower() == 'stats':
            stats = search.get_stats()
            print(f"\nIndex: {stats.get('name')}")
            print(f"Documents: {stats.get('num_documents', 0)}")
            print(f"Embeddings: {stats.get('num_embeddings', 0)}")
            continue

        # Search
        results = search.search(query, k=10)

        if not results:
            print("No results found.")
            continue

        print(f"\nFound {len(results)} results:")
        for i, result in enumerate(results[:5], 1):
            meta = result.get('metadata', {})
            file_path = meta.get('file', 'unknown')
            line = meta.get('line', 0)
            name = meta.get('name', 'unknown')
            unit_type = meta.get('unit_type', 'unknown')
            score = result.get('score', 0)

            print(f"\n{i}. {file_path}:{line} (score: {score:.3f})")
            print(f"   {name} ({unit_type})")

            # Show code snippet if available
            code = meta.get('code', '')
            if code:
                snippet = code[:150].replace('\n', ' ')
                if len(code) > 150:
                    snippet += "..."
                print(f"   {snippet}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Remote GPU + NextPlaid code search example"
    )
    parser.add_argument(
        "--server", "-s",
        default=os.getenv("REMOTE_GPU_URL", "http://localhost:8080"),
        help="Remote GPU server URL (default: http://localhost:8080)"
    )
    parser.add_argument(
        "--index", "-i",
        default=os.getenv("INDEX_NAME", "remote_project"),
        help="Index name (default: remote_project)"
    )
    parser.add_argument(
        "--project", "-p",
        default=os.getenv("PROJECT_PATH", "."),
        help="Project path to index (default: current directory)"
    )
    parser.add_argument(
        "--interactive", "-I",
        action="store_true",
        help="Interactive search mode"
    )

    args = parser.parse_args()

    # Set environment for main()
    os.environ["REMOTE_GPU_URL"] = args.server
    os.environ["INDEX_NAME"] = args.index
    os.environ["PROJECT_PATH"] = args.project

    if args.interactive:
        interactive_mode(args.server, args.index)
    else:
        main()
