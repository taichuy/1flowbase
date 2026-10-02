import copy
import unittest
from ci_observation_schema import review

def fixture(exits=True):
    o={"probe_id":"fixture","raw_data":[{"receive_ns":"100000000","bytes":100},{"receive_ns":"2200000000","bytes":200}],
       "delta_trace":[{"delta_index":0,"parse_ns":"110000000"},{"delta_index":1,"parse_ns":"2210000000"}]}
    if exits:
        o["raw_data"][0]["callback_exit_ns"]="120000000"
        o["raw_data"][1]["callback_exit_ns"]="2220000000"
    c={"probe_id":"fixture","correlation_evidence":"explicit_same_stream_and_delta_sequence",
       "clock_id":"fake-clock","source_clock_id":"fake-clock","client_clock_id":"fake-clock",
       "max_clock_error_ns":"0","delta_sequence":[0,1],"source_send_ns":["90000000","100000000"],
       "client_receive_ns":["110000000","2210000000"],"source_boundary":"before_res_write","client_boundary":"SSE_delta_parse_callback"}
    return o,c

class SchemaTests(unittest.TestCase):
    def test_current_ci_string_clock_and_missing_callback_exit(self):
        o,c=fixture(False);v=review(o,c)
        self.assertEqual(v["status"],"boundary_review")
        self.assertEqual(v["callback_duration_evidence"],"missing_callback_exit_ns")
        self.assertEqual(v["largest_gap_review"]["parse_gap_ms"],2100)
    def test_complete_exact_callback_partition(self):
        o,c=fixture();v=review(o,c)
        self.assertEqual(v["largest_gap_review"]["partition_ms"],{"previous_callback_remaining":10,"between_callbacks":2080,"next_callback_before_parse":10})
    def test_long_previous_callback_not_socket_gap(self):
        o,c=fixture();o["raw_data"][0]["callback_exit_ns"]="2190000000"
        v=review(o,c)["largest_gap_review"]["partition_ms"]
        self.assertEqual(v["previous_callback_remaining"],2080)
        self.assertEqual(v["between_callbacks"],10)
    def test_same_callback_parse_pause(self):
        o,c=fixture();o["raw_data"]=o["raw_data"][:1];o["raw_data"][0]["callback_exit_ns"]="2220000000"
        v=review(o,c)["largest_gap_review"]
        self.assertTrue(v["same_raw_callback"]);self.assertEqual(v["raw_callback_gap_ms"],0)
        self.assertEqual(v["partition_ms"]["within_same_callback"],2100)
    def test_timer_delayed_after_gap_is_counted(self):
        o,c=fixture();rows=[{"timestamp_ns":"100000000","lag_ns":"0"},{"timestamp_ns":"2230000000","lag_ns":"2000000000"}]
        v=review(o,c,rows)["largest_gap_review"]["loop_probe"]
        self.assertTrue(v["brackets_gap"]);self.assertEqual(v["overlapping_timer_lag_max_ms"],2000)
    def test_missing_lag_not_zero(self):
        o,c=fixture();v=review(o,c)["largest_gap_review"]["loop_probe"]
        self.assertFalse(v["brackets_gap"]);self.assertIsNone(v["overlapping_timer_lag_max_ms"])
    def test_mapping_timestamp_and_overlap_negatives(self):
        for mode in ("probe","parse","overlap","clock"):
            o,c=fixture()
            if mode=="probe":c["probe_id"]="other"
            if mode=="parse":c["client_receive_ns"][1]="2209000000"
            if mode=="overlap":o["raw_data"][0]["callback_exit_ns"]="2230000000"
            if mode=="clock":c["max_clock_error_ns"]=0.1
            self.assertEqual(review(o,c)["status"],"invalid_or_incomplete",mode)

if __name__=="__main__":unittest.main()
