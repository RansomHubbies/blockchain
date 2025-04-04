"""
Example Django views for integrating with ChatChain.

This is a simplified example demonstrating how Django views could
interact with our ChatChain blockchain application.
"""

import json
import time
from typing import Any, Dict, List

import requests
from django.http import JsonResponse
from django.views.decorators.csrf import csrf_exempt
from django.views.decorators.http import require_http_methods

# Configuration
CHATCHAIN_API_URL = "http://127.0.0.1:8081"


def send_to_mempool(message: Dict[str, Any]) -> Dict[str, Any]:
    """Send a message to the ChatChain mempool."""
    response = requests.post(
        f"{CHATCHAIN_API_URL}/mempool/push",
        json=message,
        headers={"Content-Type": "application/json"},
    )
    return {"status": response.status_code, "response": response.json()}


def commit_messages() -> Dict[str, Any]:
    """Trigger a commit of all messages in the mempool."""
    response = requests.post(f"{CHATCHAIN_API_URL}/commit")
    return {"status": response.status_code, "response": response.json()}


def get_messages() -> List[Dict[str, Any]]:
    """Retrieve all messages from the blockchain."""
    response = requests.get(f"{CHATCHAIN_API_URL}/messages")
    if response.status_code == 200:
        return response.json()
    return []


# Django views
@csrf_exempt
@require_http_methods(["POST"])
def django_send_message(request):
    """Django view for sending a new message."""
    try:
        # Parse request data
        data = json.loads(request.body)
        sender = data.get("sender")
        recipient = data.get("recipient")
        text = data.get("text")

        # Validate required fields
        if not sender or not recipient or not text:
            return JsonResponse({"error": "Missing required fields"}, status=400)

        # Create message payload
        message = {
            "sender": sender,
            "recipient": recipient,
            "text": text,
            "timestamp": int(time.time()),
        }

        # Send to mempool
        result = send_to_mempool(message)

        # Optionally, trigger an immediate commit
        # This could be done asynchrously or on a schedule instead
        commit_result = commit_messages()

        return JsonResponse(
            {
                "status": "success",
                "message": "Message sent to blockchain",
                "details": {
                    "mempool_result": result,
                    "commit_result": commit_result,
                },
            }
        )

    except json.JSONDecodeError:
        return JsonResponse({"error": "Invalid JSON"}, status=400)
    except Exception as e:
        return JsonResponse({"error": str(e)}, status=500)


@require_http_methods(["GET"])
def django_get_messages(request):
    """Django view for retrieving all messages."""
    try:
        # Get recipient filter if provided
        recipient = request.GET.get("recipient")

        # Get all messages from the blockchain
        messages = get_messages()

        # Filter by recipient if specified
        if recipient:
            messages = [msg for msg in messages if msg["recipient"] == recipient]

        return JsonResponse(
            {
                "status": "success",
                "messages": messages,
            }
        )

    except Exception as e:
        return JsonResponse({"error": str(e)}, status=500)


# Example of how these views would be wired up in Django urls.py:
"""
from django.urls import path
from . import views

urlpatterns = [
    path('api/messages/send', views.django_send_message, name='send_message'),
    path('api/messages/list', views.django_get_messages, name='get_messages'),
]
"""


# Demonstration of how these functions could be used directly
if __name__ == "__main__":
    # Example usage
    print("Sending a test message to the blockchain...")

    test_message = {
        "sender": "DjangoApp",
        "recipient": "TestUser",
        "text": "Hello from the Django client example!",
        "timestamp": int(time.time()),
    }

    result = send_to_mempool(test_message)
    print(f"Message sent to mempool: {result}")

    commit_result = commit_messages()
    print(f"Messages committed: {commit_result}")

    messages = get_messages()
    print(f"Messages from blockchain: {json.dumps(messages, indent=2)}")
