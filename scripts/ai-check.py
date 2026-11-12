# Is the smart bar's model reachable with couch's key? Runs the same kind of
# two-step tool call the Vibe page makes and prints what came back (never the key).
import json, os, sys, urllib.request, urllib.error

KEY = os.environ.get("COUCH_AI_KEY", "")
URL = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
if not KEY:
    print("no COUCH_AI_KEY"); sys.exit(0)

def chat(model, msgs, tools=None, json_out=False):
    body = {"model": model, "messages": msgs, "max_tokens": 4096}
    if tools: body["tools"] = tools
    if json_out: body["response_format"] = {"type": "json_object"}
    req = urllib.request.Request(URL, json.dumps(body).encode(), {"Authorization": "Bearer " + KEY, "Content-Type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=90) as r:
            return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        return e.code, e.read().decode()[:400]

tools = [{"type": "function", "function": {"name": "get_taste_details", "description": "the person's full watch history", "parameters": {"type": "object", "properties": {}}}}]
for model in ["gemini-flash-latest", "gemini-2.5-flash", "gemini-2.5-flash-lite"]:
    msgs = [{"role": "system", "content": "Recommend 3 titles. Call get_taste_details first, then answer as JSON {\"picks\":[{\"title\":..,\"year\":..}]}."},
            {"role": "user", "content": "a heist movie that's actually smart"}]
    st, d = chat(model, msgs, tools)
    print(model, "round1", st, (json.dumps(d)[:300] if st == 200 else d))
    if st != 200: continue
    m = d["choices"][0]["message"]
    print("  finish:", d["choices"][0].get("finish_reason"), "tool_calls:", bool(m.get("tool_calls")), "content:", (m.get("content") or "")[:80])
    if m.get("tool_calls"):
        msgs.append(m)
        for c in m["tool_calls"]:
            msgs.append({"role": "tool", "tool_call_id": c.get("id"), "name": c["function"]["name"], "content": json.dumps({"loved": ["Inception", "Ocean's Eleven"]})})
        st, d = chat(model, msgs, tools)
        print("  round2", st, (json.dumps(d)[:300] if st == 200 else d))
    st, d = chat(model, msgs[:2], None, True)
    print("  json-only", st, (json.dumps(d)[:200] if st == 200 else d))
