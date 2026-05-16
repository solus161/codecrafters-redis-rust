#!/bin/bash
{
  # printf "*1\r\n\$5\r\nMULTI\r\n"
  # printf "*2\r\n\$5\r\nWATCH\r\n\$9\r\npineapple\r\n"

  # printf "*3\r\n\$3\r\nSET\r\n\$9\r\nraspberry\r\n\$2\r\n38\r\n"
  # printf "*3\r\n\$3\r\nSET\r\n\$9\r\nblueberry\r\n\$2\r\n41\r\n"
  printf "*2\r\n\$5\r\nWATCH\r\n\$9\r\nraspberry\r\n"
  printf "*1\r\n\$5\r\nMULTI\r\n"
  printf "*3\r\n\$3\r\nSET\r\n\$9\r\nblueberry\r\n\$2\r\n87\r\n"
  sleep 1
  printf "*3\r\n\$3\r\nSET\r\n\$9\r\nraspberry\r\n\$3\r\n111\r\n"
  printf "*1\r\n\$4\r\nEXEC\r\n"
} | nc localhost 6379 & CLIENT1_PID=$!

{
  printf "*3\r\n\$3\r\nSET\r\n\$9\r\nraspberry\r\n\$3\r\n111\r\n"
  sleep 1
} | nc localhost 6379 &

wait $CLIENT1_PID
wait

