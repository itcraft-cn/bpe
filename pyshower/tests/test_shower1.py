import os, sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "../lib/python3.11/site-packages")
)
sys.path.insert(
    0, os.path.join(os.path.dirname(__file__), "../lib64/python3.11/site-packages")
)

from pyshower.shower import Shower,ShowerRecordCallback
import pytest
    
SQL = "select _1.__1 from _1 limit 1"

class XDemo(ShowerRecordCallback):
    def __init__(self):
        super().__init__()

    def callback(self, data):
        print(data)


class TestUniStream:
    # 函数级开始
    def setup_method(self):
        os.environ['SHOWER_HOME'] = '/home/helly/code/rust/shower'
        Shower.start()

    # 函数级结束
    def teardown_method(self):
        Shower.stop()

    # 测试
    def test(self):
        callback = XDemo()
        global SQL
        Shower.def_action_with_callback(SQL, callback)
        for i in range(100):
            Shower.new_data(1, b'000000000000000000000000')


if __name__ == "__main__":
    pytest.main()
