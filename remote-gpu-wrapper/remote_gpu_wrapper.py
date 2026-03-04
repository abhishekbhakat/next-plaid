"""
Remote GPU + NextPlaid Wrapper

Connects to a self-hosted NextPlaid API server running on a remote GPU machine
(e.g., NVIDIA 4070 Super). Your laptop handles parsing; the remote server handles
all encoding, indexing, and search.
"""

import os
import sys
from pathlib import Path
from typing import List, Dict, Any, Optional, Union
import requests
import json

# Add parent directory for parser import
sys.path.insert(0, str(Path(__file__).parent.parent / "baseten-wrapper"))

try:
    from parser import CodeParser
except ImportError:
    raise ImportError("Parser module not found. Ensure baseten-wrapper/parser.py exists")


class RemoteGPUEncoder:
    """
    Client for remote NextPlaid API with GPU encoding.
    """

    def __init__(self, server_url: str, timeout: float = 300.0):
        self.server_url = server_url.rstrip("/")
        self.timeout = timeout

    def _request(self, method: str, endpoint: str, json_data: dict = None) -> dict:
        """Make HTTP request to NextPlaid API."""
        url = f"{self.server_url}{endpoint}"
        response = requests.request(
            method=method,
            url=url,
            json=json_data,
            timeout=self.timeout
        )
        response.raise_for_status()
        return response.json()

    def health(self) -> dict:
        """Check server health."""
        return self._request("GET", "/health")

    def encode_texts(
        self,
        texts: List[str],
        input_type: str = "document",
        pool_factor: Optional[int] = None
    ) -> List[List[List[float]]]:
        """
        Encode texts using remote GPU.

        Args:
            texts: List of texts to encode
            input_type: "document" or "query"
            pool_factor: Optional pooling factor

        Returns:
            Multi-vector embeddings
        """
        payload = {
            "texts": texts,
            "input_type": input_type
        }
        if pool_factor is not None:
            payload["pool_factor"] = pool_factor

        result = self._request("POST", "/encode", payload)
        return result["embeddings"]

    def encode_queries(self, queries: List[str]) -> List[List[List[float]]]:
        """Encode search queries."""
        return self.encode_texts(queries, input_type="query")

    def encode_documents(self, documents: List[str], pool_factor: Optional[int] = None) -> List[List[List[float]]]:
        """Encode code documents."""
        return self.encode_texts(documents, input_type="document", pool_factor=pool_factor)


class RemoteGPUCodeSearch:
    """
    Semantic code search using a remote GPU server running NextPlaid API.

    This combines:
    - Local Tree-sitter parsing (your laptop)
    - Remote GPU encoding (NextPlaid API on GPU server)
    - Remote indexing and search (NextPlaid on GPU server)

    The entire index lives on the remote server, making this ideal when you have
    a powerful GPU machine but want to search from a lightweight laptop.
    """

    def __init__(
        self,
        server_url: str = "http://localhost:8080",
        index_name: str = "default",
        timeout: float = 300.0,
    ):
        """
        Initialize the remote GPU code search.

        Args:
            server_url: URL of remote NextPlaid API server
            index_name: Name for the search index
            timeout: HTTP request timeout in seconds
        """
        self.server_url = server_url.rstrip("/")
        self.index_name = index_name
        self.encoder = RemoteGPUEncoder(server_url, timeout)
        self.parser = CodeParser()

        # Ensure index exists
        self._ensure_index()

    def _request(self, method: str, endpoint: str, json_data: dict = None) -> dict:
        """Make HTTP request to NextPlaid API."""
        url = f"{self.server_url}{endpoint}"
        response = requests.request(
            method=method,
            url=url,
            json=json_data,
            timeout=self.encoder.timeout
        )
        response.raise_for_status()
        return response.json()

    def _ensure_index(self):
        """Create index if it doesn't exist."""
        try:
            self._request("GET", f"/indices/{self.index_name}")
        except requests.exceptions.HTTPError as e:
            if e.response.status_code == 404:
                # Create index
                self._request("POST", "/indices", {
                    "name": self.index_name,
                    "config": {"nbits": 4}
                })
                print(f"Created index: {self.index_name}")
            else:
                raise

    def index_project(
        self,
        project_path: str,
        exclude_dirs: Optional[List[str]] = None,
        batch_size: int = 64,
        pool_factor: Optional[int] = None,
    ) -> int:
        """
        Index a project directory.

        Args:
            project_path: Path to project root
            exclude_dirs: Directories to exclude
            batch_size: Documents per batch
            pool_factor: Optional embedding pooling factor

        Returns:
            Number of code units indexed
        """
        # Parse code locally
        print(f"Parsing project: {project_path}")
        code_units = self.parser.parse_project(project_path, exclude_dirs=exclude_dirs)
        print(f"Found {len(code_units)} code units")

        if not code_units:
            return 0

        # Index in batches
        total_indexed = 0
        for i in range(0, len(code_units), batch_size):
            batch = code_units[i:i + batch_size]
            self._index_batch(batch, pool_factor)
            total_indexed += len(batch)
            print(f"Indexed {total_indexed}/{len(code_units)} units...")

        print(f"Indexing complete! Total: {total_indexed} units")
        return total_indexed

    def _index_batch(self, code_units: List[Dict[str, Any]], pool_factor: Optional[int] = None):
        """Index a batch of code units."""
        # Prepare documents and metadata
        documents = [unit["text"] for unit in code_units]
        metadata = [
            {
                "file": unit.get("file", ""),
                "line": unit.get("line", 0),
                "name": unit.get("name", ""),
                "unit_type": unit.get("unit_type", ""),
                "code": unit.get("code", "")[:1000],  # Store first 1000 chars
            }
            for unit in code_units
        ]

        # Send to remote server for encoding + indexing
        payload = {
            "documents": documents,
            "metadata": metadata,
        }
        if pool_factor is not None:
            payload["pool_factor"] = pool_factor

        self._request(
            "POST",
            f"/indices/{self.index_name}/update_with_encoding",
            payload
        )

    def search(
        self,
        query: str,
        k: int = 10,
        filter_condition: Optional[str] = None,
        filter_parameters: Optional[List[Any]] = None,
    ) -> List[Dict[str, Any]]:
        """
        Semantic search over indexed code.

        Args:
            query: Search query text
            k: Number of results
            filter_condition: Optional SQL filter
            filter_parameters: Filter parameters

        Returns:
            List of search results
        """
        payload = {
            "queries": [query],
            "params": {"top_k": k}
        }

        if filter_condition:
            payload["filter_condition"] = filter_condition
            if filter_parameters:
                payload["filter_parameters"] = filter_parameters

            endpoint = f"/indices/{self.index_name}/search/filtered_with_encoding"
        else:
            endpoint = f"/indices/{self.index_name}/search_with_encoding"

        result = self._request("POST", endpoint, payload)

        # Format results
        formatted = []
        if result.get("results"):
            for r in result["results"][0].get("results", []):
                formatted.append({
                    "score": r.get("score", 0),
                    "document_id": r.get("document_id", -1),
                    "metadata": r.get("metadata", {}),
                })

        return formatted

    def search_with_code(
        self,
        query: str,
        k: int = 10,
    ) -> List[Dict[str, Any]]:
        """
        Search and return full code snippets.

        Args:
            query: Search query
            k: Number of results

        Returns:
            Results with full code included
        """
        results = self.search(query, k=k)

        # Fetch full metadata for each result
        for r in results:
            doc_id = r["document_id"]
            try:
                meta_result = self._request(
                    "POST",
                    f"/indices/{self.index_name}/metadata/get",
                    {"document_ids": [doc_id]}
                )
                if meta_result.get("metadata"):
                    r["full_metadata"] = meta_result["metadata"][0]
            except Exception:
                pass

        return results

    def delete_index(self):
        """Delete the entire index."""
        self._request("DELETE", f"/indices/{self.index_name}")
        print(f"Deleted index: {self.index_name}")

    def get_stats(self) -> Dict[str, Any]:
        """Get index statistics."""
        try:
            info = self._request("GET", f"/indices/{self.index_name}")
            return {
                "name": info.get("name"),
                "num_documents": info.get("num_documents", 0),
                "num_embeddings": info.get("num_embeddings", 0),
                "dimension": info.get("dimension", 0),
            }
        except Exception as e:
            return {"error": str(e)}

    def health_check(self) -> Dict[str, Any]:
        """Check health of remote server."""
        try:
            health = self.encoder.health()
            return {
                "status": "healthy",
                "version": health.get("version"),
                "model": health.get("model", {}).get("name", "none"),
                "loaded_indices": health.get("loaded_indices", 0),
                "gpu_available": "cuda" in health.get("model", {}).get("name", "").lower() or
                               health.get("model", {}).get("name", "") != "none",
            }
        except Exception as e:
            return {"status": "error", "error": str(e)}

    def list_indices(self) -> List[str]:
        """List all indices on remote server."""
        return self._request("GET", "/indices")
