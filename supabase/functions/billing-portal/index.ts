// POST (signed-in user) -> {url} of Stripe's customer portal: update card, see invoices, cancel.
// Secrets: STRIPE_SECRET_KEY, SITE_URL. Turn the portal on once in Stripe: Settings > Billing > Customer portal.
import { createClient } from "npm:@supabase/supabase-js@2";
Deno.serve(async (req) => {
  const sb = createClient(Deno.env.get("SUPABASE_URL")!, Deno.env.get("SUPABASE_ANON_KEY")!, {
    global: { headers: { Authorization: req.headers.get("Authorization")! } },
  });
  const { data: { user } } = await sb.auth.getUser();
  if (!user) return new Response("sign in first", { status: 401 });
  const { data } = await sb.from("subscriptions").select("stripe_customer").eq("user_id", user.id).maybeSingle();
  if (!data?.stripe_customer) return Response.json({ error: "no billing yet: subscribe first" }, { status: 404 });
  const r = await fetch("https://api.stripe.com/v1/billing_portal/sessions", {
    method: "POST",
    body: new URLSearchParams({ customer: data.stripe_customer, return_url: Deno.env.get("SITE_URL")! }),
    headers: { Authorization: `Bearer ${Deno.env.get("STRIPE_SECRET_KEY")}`, "Content-Type": "application/x-www-form-urlencoded" },
  });
  const s = await r.json();
  return Response.json({ url: s.url }, { status: s.url ? 200 : 502 });
});
