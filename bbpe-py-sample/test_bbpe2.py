import os
from pybbpe.bbpe import Bbpe, BbpeRecordCallback

SQL = "select demo.a from demo limit 10"
SQL2 = "select _suml(stream.a) from stream"


class XDemo(BbpeRecordCallback):
    def __init__(self):
        super().__init__()

    def callback(self, data):
        print(data)


class TestBbpe:
    # 函数级开始
    def setup_method(self):
        os.environ["BBPE_HOME"] = "/home/helly/code/rust/bbpe"
        Bbpe.start()

    # 函数级结束
    def teardown_method(self):
        Bbpe.stop()

    # 测试
    def test(self):
        callback = XDemo()
        Bbpe.def_incoming("demo", ["a"], [0], [0])
        Bbpe.def_stream("stream", ["a"], [0], [0])
        global SQL
        global SQL2
        aggregate_id = Bbpe.def_aggregate(SQL2, callback)
        mapper_id = Bbpe.def_mapper_bind_aggregate(SQL, aggregate_id)
        print("mapper_id:", mapper_id, "aggregate_id:", aggregate_id)
        for i in range(100):
            Bbpe.new_data_sync(1, b"100000000000000000000000")


if __name__ == "__main__":
    a = TestBbpe()
    a.setup_method()
    a.test()
    a.teardown_method()
