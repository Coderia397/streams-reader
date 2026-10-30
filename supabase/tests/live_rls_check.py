"""Live behaviour check of the Supabase schema (supabase/migrations) with throwaway users.

Run after `cargo xtask supabase-migrate`:  python3 -I supabase/tests/live_rls_check.py
Standard library only. Reads the Management API token from ~/.config/streams/supabase.env and fetches the project's
keys at runtime; prints PASS/FAIL lines and never a secret. Creates users rls-*-<random>@example.com and invites,
and deletes them again. Only ever talks to project atxbmgfuxjtcwmoaldqy.
"""
import json
import os
import secrets
import urllib.error
import urllib.request

REF = "atxbmgfuxjtcwmoaldqy"
URL = f"https://{REF}.supabase.co"
results = []


def env(path):
    out = {}
    for line in open(os.path.expanduser(path)):
        line = line.strip()
        if "=" in line and not line.startswith("#"):
            k, v = line.split("=", 1)
            out[k.strip()] = v.strip().strip('"').strip("'")
    return out


TOKEN = env("~/.config/streams/supabase.env")["SUPABASE_ACCESS_TOKEN"]


def management(method, path, body=None):
    req = urllib.request.Request(f"https://api.supabase.com/v1/projects/{REF}{path}",
                                 data=None if body is None else json.dumps(body).encode(), method=method)
    req.add_header("Authorization", f"Bearer {TOKEN}")
    req.add_header("content-type", "application/json")
    req.add_header("user-agent", "nolabs-live-check/1")
    with urllib.request.urlopen(req) as r:
        return json.loads(r.read().decode() or "null")


def sql(q):
    return management("POST", "/database/query", {"query": q})


keys = management("GET", "/api-keys?reveal=true")
PUB = next(k["api_key"] for k in keys if k.get("type") == "publishable")
SRV = next(k["api_key"] for k in keys if k["name"] == "service_role" and k.get("type") == "legacy")


def http(method, path, body=None, bearer=None, apikey=PUB, prefer=None):
    req = urllib.request.Request(URL + path, data=None if body is None else json.dumps(body).encode(), method=method)
    req.add_header("apikey", apikey)
    req.add_header("content-type", "application/json")
    if bearer:
        req.add_header("authorization", "Bearer " + bearer)
    if prefer:
        req.add_header("prefer", prefer)
    try:
        with urllib.request.urlopen(req) as r:
            t = r.read().decode()
            return r.status, (json.loads(t) if t.strip() else None)
    except urllib.error.HTTPError as e:
        t = e.read().decode()
        try:
            return e.code, json.loads(t)
        except ValueError:
            return e.code, t[:200]


def check(name, ok, detail=""):
    results.append((name, ok))
    print(("PASS " if ok else "FAIL ") + name + ("" if ok else f"  [{detail}]"))


def admin_create(email, meta, password=None):
    body = {"email": email, "email_confirm": True, "user_metadata": meta}
    if password:
        body["password"] = password
    return http("POST", "/auth/v1/admin/users", body, bearer=SRV, apikey=SRV)


def session(email):
    st, link = http("POST", "/auth/v1/admin/generate_link", {"type": "magiclink", "email": email}, bearer=SRV, apikey=SRV)
    assert st == 200, (st, link)
    th = link.get("hashed_token") or link.get("properties", {}).get("hashed_token")
    st, s = http("POST", "/auth/v1/verify", {"type": "magiclink", "token_hash": th})
    assert st == 200, (st, s)
    return s["access_token"], s["user"]["id"]


tag = secrets.token_hex(4)
ea, eb, ec = f"rls-a-{tag}@example.com", f"rls-b-{tag}@example.com", f"rls-c-{tag}@example.com"
code_a = sql("insert into public.invites (note) values ('rls test a') returning code")[0]["code"]
code_b = sql(f"insert into public.invites (note, email) values ('rls test b', '{eb}') returning code")[0]["code"]
users = []
try:
    st, ua = admin_create(ea, {"invite_code": code_a, "locale": "de"}, password="Very-Long-Password-123!")
    check("invited user (code) is created", st == 200, (st, ua))
    users.append(ua["id"])
    st, ub = admin_create(eb, {})
    check("user with an email-bound invite is created without a code", st == 200, (st, ub))
    users.append(ub["id"])
    st, uc = admin_create(ec, {})
    check("user without an invite is refused", st >= 400, (st, uc))
    if st == 200:
        users.append(uc["id"])
    st, ua2 = admin_create(f"rls-d-{tag}@example.com", {"invite_code": code_a})
    check("a spent invite cannot create a second user", st >= 400, st)
    if st == 200:
        users.append(ua2["id"])
    pw = sql(f"select encrypted_password from auth.users where id = '{ua['id']}'")[0]["encrypted_password"]
    check("no password is stored", pw in ("", None), "has a hash")
    st, _ = http("POST", "/auth/v1/token?grant_type=password", {"email": ea, "password": "Very-Long-Password-123!"})
    check("password sign-in fails", st == 400, st)
    prof = sql(f"select locale, is_admin from public.profiles where id = '{ua['id']}'")[0]
    check("profile created with the sign-up locale", prof == {"locale": "de", "is_admin": False}, prof)

    ta, ida = session(ea)
    tb, idb = session(eb)
    check("magic-link session minted without a password", bool(ta and tb))
    doc = "0123456789abcdef"[:12] + tag[:4].replace("i", "j").replace("l", "m").replace("o", "p").replace("u", "v")
    st, d = http("POST", "/rest/v1/documents", {"id": doc, "title": "t", "source": "paste", "body_bytes": 10}, bearer=ta, prefer="return=representation")
    check("owner inserts a document (user_id defaults to auth.uid())", st == 201 and d[0]["user_id"] == ida, (st, d))
    st, rows = http("GET", f"/rest/v1/documents?id=eq.{doc}&select=id", bearer=tb)
    check("another user cannot see it", st == 200 and rows == [], (st, rows))
    st, rows = http("PATCH", f"/rest/v1/documents?id=eq.{doc}", {"title": "hacked"}, bearer=tb, prefer="return=representation")
    check("another user cannot change it", rows == [], (st, rows))
    st, rows = http("POST", "/rest/v1/documents", {"id": doc[:15] + "z", "user_id": ida, "title": "x", "source": "paste", "body_bytes": 1}, bearer=tb)
    check("nobody can insert rows for someone else", st in (401, 403), (st, rows))
    st, r = http("POST", "/rest/v1/progress", {"document_id": doc, "char_offset": 5}, bearer=tb)
    check("progress cannot point at another user's document", st >= 400, (st, r))
    t1, t0 = "2026-10-30T12:00:00Z", "2026-10-30T11:00:00Z"
    http("POST", "/rest/v1/progress", {"document_id": doc, "char_offset": 50, "updated_at": t1}, bearer=ta, prefer="resolution=merge-duplicates")
    st, r = http("POST", "/rest/v1/progress", {"document_id": doc, "char_offset": 10, "updated_at": t0}, bearer=ta, prefer="resolution=merge-duplicates")
    st, rows = http("GET", f"/rest/v1/progress?document_id=eq.{doc}&select=char_offset", bearer=ta)
    check("an older progress write never replaces a newer one", rows == [{"char_offset": 50}], (st, rows))
    st, rows = http("PATCH", f"/rest/v1/profiles?id=eq.{ida}", {"is_admin": True}, bearer=ta)
    check("users cannot make themselves admin", st in (401, 403), (st, rows))
    st, share = http("POST", "/rest/v1/rpc/gpu_share", {"daily_cap_s": 280}, bearer=ta)
    check("an invited user's GPU share is 60 s", share == 60, (st, share))
    st, inv = http("POST", "/rest/v1/rpc/create_invite", {"days": 3}, bearer=ta)
    check("non-admins cannot create invites", st >= 400, (st, inv))
    st, rows = http("GET", "/rest/v1/invites?select=code", bearer=ta)
    check("non-admins cannot list invites", rows == [], (st, rows))
    st, s = http("POST", "/rest/v1/rpc/invite_status", {"code": code_a})
    check("anon can ask invite_status (spent code reads used)", s == "used", (st, s))
    fresh = sql("insert into public.invites (note) values ('rls test r') returning code")[0]["code"]
    st, s = http("POST", "/rest/v1/rpc/reserve_invite", {"code": fresh, "email": f"Someone-{tag}@Example.com"})
    st2, s2 = http("POST", "/rest/v1/rpc/reserve_invite", {"code": fresh, "email": f"other-{tag}@example.com"})
    check("reserve_invite binds once, refuses another email", (s, s2) == ("reserved", "taken"), (s, s2))
    sql(f"delete from public.invites where code = '{fresh}'")
    st, rows = http("GET", "/rest/v1/documents?select=id")
    check("anon reads nothing", st in (401, 403) or rows == [], (st, rows))
    st, rows = http("POST", "/rest/v1/rpc/handle_new_user", {})
    check("internal functions are not reachable", st == 404, st)
finally:
    for u in users:
        http("DELETE", f"/auth/v1/admin/users/{u}", bearer=SRV, apikey=SRV)
    sql(f"delete from public.invites where code in ('{code_a}', '{code_b}')")
    left = sql(f"select count(*) as n from auth.users where email like 'rls-%-{tag}@example.com'")[0]["n"]
    print("cleanup: users left", left)
print(f"{sum(ok for _, ok in results)}/{len(results)} passed")
