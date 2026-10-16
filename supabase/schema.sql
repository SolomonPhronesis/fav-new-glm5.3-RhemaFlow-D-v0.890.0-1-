-- RhemaFlow accounts. Run once in the Supabase SQL editor.
create table if not exists subscriptions (
  user_id uuid primary key references auth.users on delete cascade,
  stripe_customer text unique,
  status text not null default 'inactive',      -- active | trialing | past_due | canceled | inactive
  plan text not null default 'monthly',          -- monthly | annual | church
  seats int not null default 1,
  current_period_end timestamptz
);
create table if not exists devices (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references auth.users on delete cascade,
  name text not null,
  last_seen timestamptz not null default now(),
  unique (user_id, name)
);
alter table subscriptions enable row level security;
alter table devices enable row level security;
-- the app (signed-in user) may READ its own subscription; nobody but the webhook (service role) may write it
create policy "read own subscription" on subscriptions for select using (auth.uid() = user_id);
create policy "own devices" on devices for all using (auth.uid() = user_id) with check (auth.uid() = user_id);

-- ===== v0.833.0 =====
-- 14-day free trial, no card: every new account starts "trialing" for 14 days (the app already treats trialing as paid).
create or replace function handle_new_user() returns trigger language plpgsql security definer set search_path = public as $$
begin
  insert into subscriptions (user_id, status, plan, current_period_end)
  values (new.id, 'trialing', 'monthly', now() + interval '14 days')
  on conflict (user_id) do nothing;
  return new;
end $$;
drop trigger if exists on_auth_user_created on auth.users;
create trigger on_auth_user_created after insert on auth.users for each row execute function handle_new_user();

-- 2 computers per account (church plan: 5). Called by the app right after every sign-in.
create or replace function register_device(device_name text) returns text language plpgsql security definer set search_path = public as $$
declare uid uuid := auth.uid(); n int; allowed int;
begin
  if uid is null then raise exception 'not_signed_in'; end if;
  select case when plan = 'church' then 5 else 2 end into allowed from subscriptions where user_id = uid;
  allowed := coalesce(allowed, 2);
  select count(*) into n from devices where user_id = uid and name <> device_name;
  if n >= allowed then raise exception 'device_limit'; end if;
  insert into devices (user_id, name, last_seen) values (uid, device_name, now())
    on conflict (user_id, name) do update set last_seen = now();
  return 'ok';
end $$;

-- "Sign out my other computers"
create or replace function forget_other_devices(device_name text) returns text language plpgsql security definer set search_path = public as $$
begin
  delete from devices where user_id = auth.uid() and name <> device_name;
  return 'ok';
end $$;
