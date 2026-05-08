#!/bin/bash
{
  printf "*5\r\n\$4\r\nXADD\r\n\$9\r\npineapple\r\n\$3\r\n1-2\r\n\$6\r\nbanana\r\n\$9\r\nraspberry\r\n"
  printf "*5\r\n\$4\r\nXADD\r\n\$9\r\npineapple\r\n\$3\r\n1-2\r\n\$6\r\nbanana\r\n\$9\r\nraspberry\r\n"

} | nc localhost 6379
