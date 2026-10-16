// POST {plan: "monthly" | "annual" | "church"}  with the user's Supabase JWT -> {url} of a Stripe Checkout page.
// Secrets: STRIPE_SECRET_KEY, PRICE_MONTHLY (the £10/month price id), PRICE_ANNUAL, PRICE_CHURCH, SITE_URL
import { createClient } from "npm:@supabase/supabase-js@2";
const prices: Record<string, string> = {
  monthly: Deno.env.get("PRICE_MONTHLY")!, annual: Deno.env.get("PRICE_ANNUAL")!, church: Deno.env.get("PRICE_CHURCH")!,
};
Deno.serve(async (req) => {
  const sb = createClient(Deno.env.get("SUPABASE_URL")!, Deno.env.get("SUPABASE_ANON_KEY")!, {
    global: { headers: { Authorization: req.headers.get("Authorization")! } },
  });
  const { data: { user } } = await sb.auth.getUser();
  if (!user) return new Response("sign in first", { status: 401 });
  const { plan } = await req.json();
  const price = prices[plan];
  if (!price) return new Response("unknown plan", { status: 400 });
  const body = new URLSearchParams({
    mode: "subscription", "line_items[0][price]": price, "line_items[0][quantity]": "1",
    client_reference_id: user.id, customer_email: user.email ?? "",
    "subscription_data[metadata][plan]": plan, "subscription_data[metadata][user_id]": user.id,
    success_url: `${Deno.env.get("SITE_URL")}/thanks`, cancel_url: `${Deno.env.get("SITE_URL")}/`,
  });
  const r = await fetch("https://api.stripe.com/v1/checkout/sessions", {
    method: "POST", body,
    headers: { Authorization: `Bearer ${Deno.env.get("STRIPE_SECRET_KEY")}`, "Content-Type": "application/x-www-form-urlencoded" },
  });
  const s = await r.json();
  return Response.json({ url: s.url }, { status: s.url ? 200 : 502 });
});
