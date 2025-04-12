import os, sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "../lib/python3.11/site-packages")
)
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "../lib64/python3.11/site-packages")
)

from pybambootube.bambootube import BambooTube, BambooTubeRecordCallback
import pytest

SQL = "select demo.a from demo limit 10"
SQL2 = "select _suml(stream.a) from stream"


class XDemo(BambooTubeRecordCallback):
    def __init__(self):
        super().__init__()

    def callback(self, data):
        print(data)


class TestBambooTube:
    # 函数级开始
    def setup_method(self):
        os.environ["BAMBOOTUBE_HOME"] = "/home/helly/code/rust/bambootube"
        BambooTube.start()

    # 函数级结束
    def teardown_method(self):
        BambooTube.stop()

    # 测试
    def test(self):
        callback = XDemo()
        BambooTube.def_incoming("demo", ["a"], [0], [0])
        BambooTube.def_stream("stream", ["a"], [0], [0])
        global SQL
        global SQL2
        aggregate_id = BambooTube.def_aggregate(SQL2, callback)
        mapper_id = BambooTube.def_mapper_bind_aggregate(SQL, aggregate_id)
        print("mapper_id:", mapper_id, "aggregate_id:", aggregate_id)
        for i in range(100):
            BambooTube.new_data_sync(1, b"100000000000000000000000")


if __name__ == "__main__":
    pytest.main()
