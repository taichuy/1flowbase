WITH bounds AS (
 SELECT '{{ node-report-range.result.started_from }}'::timestamptz AS started_from,
        '{{ node-report-range.result.started_to }}'::timestamptz AS started_to,
        '{{ node-report-range.result.bucket }}'::text AS bucket
), logs AS MATERIALIZED (
 SELECT l.* FROM model_provider_request_logs l CROSS JOIN bounds b
 WHERE l.scope_id = '00000000-0000-0000-0000-000000000001'::uuid
 AND l.started_at >= b.started_from AND l.started_at < b.started_to
), totals AS (
 SELECT count(*) AS request_count,
 count(*) FILTER (WHERE total_tokens IS NOT NULL) AS usage_recorded_count,
 CASE WHEN count(*)=0 THEN 0 ELSE sum(total_tokens) END AS total_tokens,
 CASE WHEN count(*)=0 THEN 0 ELSE sum(input_tokens) END AS input_tokens,
 CASE WHEN count(*)=0 THEN 0 ELSE sum(output_tokens) END AS output_tokens,
 CASE WHEN count(*)=0 THEN 0 ELSE sum(input_cache_hit_tokens) END AS input_cache_hit_tokens,
 CASE WHEN count(*)=0 THEN 0 ELSE sum(cache_write_tokens) END AS cache_write_tokens,
 count(*) FILTER (WHERE total_cost IS NULL) AS unbilled_count
 FROM logs
), costs AS (
 SELECT currency_code, sum(total_cost)::text AS total_cost,
 count(*) FILTER (WHERE total_cost IS NULL) AS unbilled_count FROM logs GROUP BY currency_code
), user_costs AS (
 SELECT user_id,currency_code,sum(total_cost)::text AS total_cost,
 count(*) FILTER (WHERE total_cost IS NULL) AS unbilled_count
 FROM logs GROUP BY user_id,currency_code
), users AS (
 SELECT user_id,max(user_account) AS user_account,count(*) AS request_count,
 sum(total_tokens) AS total_tokens,sum(input_tokens) AS input_tokens,sum(output_tokens) AS output_tokens,
 sum(input_cache_hit_tokens) AS input_cache_hit_tokens,sum(cache_write_tokens) AS cache_write_tokens,
 count(*) FILTER (WHERE total_cost IS NULL) AS unbilled_count,
 (SELECT jsonb_agg(jsonb_build_object('currency_code',c.currency_code,'total_cost',c.total_cost,'unbilled_count',c.unbilled_count) ORDER BY c.currency_code)
 FROM user_costs c WHERE c.user_id IS NOT DISTINCT FROM l.user_id) AS costs
 FROM logs l GROUP BY user_id
), bucket_totals AS (
 SELECT date_trunc(b.bucket,l.started_at AT TIME ZONE 'Asia/Shanghai') AS bucket_start,
 count(*) AS request_count,sum(input_tokens) AS input_tokens,sum(output_tokens) AS output_tokens,
 sum(total_tokens) AS total_tokens,sum(input_cache_hit_tokens) AS input_cache_hit_tokens,
 sum(cache_write_tokens) AS cache_write_tokens,
 -- Weight the recorded provider hit rates by input volume. Keep unknown rates absent.
 sum(input_cache_hit_rate * input_tokens) FILTER (WHERE input_cache_hit_rate IS NOT NULL)
 / nullif(sum(input_tokens) FILTER (WHERE input_cache_hit_rate IS NOT NULL),0) AS input_cache_hit_rate
 FROM logs l CROSS JOIN bounds b GROUP BY 1
), buckets AS (
 SELECT generate_series(date_trunc(bucket,started_from AT TIME ZONE 'Asia/Shanghai'),
 date_trunc(bucket,(started_to - interval '1 microsecond') AT TIME ZONE 'Asia/Shanghai'),
 CASE bucket WHEN 'hour' THEN interval '1 hour' ELSE interval '1 day' END) AS bucket_start FROM bounds
), trend AS (
 SELECT b.bucket_start AT TIME ZONE 'Asia/Shanghai' AS bucket_start,
 coalesce(t.request_count,0) AS request_count,
 CASE WHEN t.request_count IS NULL THEN 0 ELSE t.input_tokens END AS input_tokens,
 CASE WHEN t.request_count IS NULL THEN 0 ELSE t.output_tokens END AS output_tokens,
 CASE WHEN t.request_count IS NULL THEN 0 ELSE t.total_tokens END AS total_tokens,
 CASE WHEN t.request_count IS NULL THEN 0 ELSE t.input_cache_hit_tokens END AS input_cache_hit_tokens,
 CASE WHEN t.request_count IS NULL THEN 0 ELSE t.cache_write_tokens END AS cache_write_tokens,
 t.input_cache_hit_rate
 FROM buckets b LEFT JOIN bucket_totals t USING(bucket_start)
)
SELECT to_jsonb(totals) || jsonb_build_object(
 'started_from',bounds.started_from,'started_to',bounds.started_to,'bucket',bounds.bucket,
 'timezone','Asia/Shanghai','generated_at',now(),
 'costs',coalesce((SELECT jsonb_agg(to_jsonb(costs) ORDER BY currency_code) FROM costs),'[]'::jsonb),
 'users',coalesce((SELECT jsonb_agg(to_jsonb(users) ORDER BY total_tokens DESC NULLS LAST,user_id) FROM users),'[]'::jsonb),
 'trend',coalesce((SELECT jsonb_agg(to_jsonb(trend) ORDER BY bucket_start) FROM trend),'[]'::jsonb)
) AS report FROM totals CROSS JOIN bounds;
