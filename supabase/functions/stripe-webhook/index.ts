// Stripe -> subscriptions table. Deploy with --no-verify-jwt; Stripe signs each call and we check it.
// Secrets: STRIPE_SECRET_KEY, STRIPE_WEBHOOK_SECRET, SUPABASE_SERVICE_ROLE_KEY
// Stripe events to send: checkout.session.completed, customer.subscription.updated, customer.subscription.deleted
import Stripe from "npm:stripe@16";
import { createClient } from "npm:@supabase/supabase-js@2";
const stripe = new Stripe(Deno.env.get("STRIPE_SECRET_KEY")!);
const db = createClient(Deno.env.get("SUPABASE_URL")!, Deno.env.get("SUPABASE_SERVICE_ROLE_KEY")!);
Deno.serve(async (req) => {
  let ev: Stripe.Event;
  try {
    ev = await stripe.webhooks.constructEventAsync(await req.text(), req.headers.get("stripe-signature")!, Deno.env.get("STRIPE_WEBHOOK_SECRET")!);
  } catch { return new Response("bad signature", { status: 400 }); }
  if (ev.type.startsWith("customer.subscription.")) {
    const s = ev.data.object as Stripe.Subscription;
    const user_id = s.metadata.user_id;
    if (user_id) {
      await db.from("subscriptions").upsert({
        user_id, stripe_customer: String(s.customer), status: s.status,
        plan: s.metadata.plan ?? "monthly",
        current_period_end: new Date(s.current_period_end * 1000).toISOString(),
      });
    }
  }
  return new Response("ok");
});
