import os
from pybpe.bpe import Bpe, BpeRecordCallback

SQL = "select demo.a from demo limit 10"


class XDemo(BpeRecordCallback):
    def __init__(self):
        super().__init__()

    def callback(self, data):
        print(data)


class TestBpe:
    # 函数级开始
    def setup_method(self):
        os.environ["BPE_HOME"] = "/home/helly/code/rust/bpe"
        Bpe.start()

    # 函数级结束
    def teardown_method(self):
        Bpe.stop()

    # 测试
    def test(self):
        callback = XDemo()
        Bpe.def_incoming("demo", ["a"], [0], [0])
        global SQL
        mapper_id = Bpe.def_mapper(SQL, callback)
        print("mapper_id:", mapper_id)
        for i in range(100):
            Bpe.new_data_sync(1, b"100000000000000000000000")


if __name__ == "__main__":
    a = TestBpe()
    a.setup_method()
    a.test()
    a.teardown_method()
